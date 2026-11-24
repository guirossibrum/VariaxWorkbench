"""Compare offline Rust and original Workbench MIDI write transcripts."""

import argparse
import os
from pathlib import Path
import struct
import subprocess
import tempfile
from unicorn import Uc, UC_ARCH_X86, UC_HOOK_CODE, UC_MODE_32
from unicorn.x86_const import UC_X86_REG_EAX, UC_X86_REG_ESP

from verify_factory_programs import (
    ARCHIVE_SHA256, SLOTS, ROOT, CoefficientOracle, digest, load_models, original_programs,
)



class OriginalChunker:
    """Execute the original macOS i386 send path with a recording sink."""

    SET_SIZE = 0x38A6
    SEND = 0x3A0A
    ENGINE = 0x100000
    DEVICE = 0x101000
    VTABLE = 0x102000
    PAYLOAD = 0x103000
    SINK = 0x106000
    RETURN = 0x106001
    STACK = 0x107F00

    def __init__(self, artifact_root):
        binary = artifact_root / 'analysis/extracted/macho-slices/L6MIDIJava.jnilib.i386.macho'
        if digest(binary.read_bytes()) != '902713ed5e4819021a73ff8a4f2d6a35521a99a96af7253dc53ef153e3789a8f':
            raise ValueError('unexpected Workbench i386 MIDI adapter SHA-256')
        self.cpu = Uc(UC_ARCH_X86, UC_MODE_32)
        # The pinned Mach-O __TEXT segment uses matching file and virtual offsets.
        self.cpu.mem_map(0x3000, 0x1000)
        self.cpu.mem_write(0x3000, binary.read_bytes()[0x3000:0x4000])
        self.cpu.mem_map(self.ENGINE, 0x8000)
        self.cpu.mem_write(self.ENGINE + 4, struct.pack('<I', self.DEVICE))
        self.cpu.mem_write(self.DEVICE, struct.pack('<I', self.VTABLE))
        self.cpu.mem_write(self.VTABLE + 0x38, struct.pack('<I', self.SINK))
        self.cpu.mem_write(self.SINK, b'\xc3')
        self.cpu.hook_add(UC_HOOK_CODE, self.capture, begin=self.SINK, end=self.SINK)
        self.records = []

    def call(self, address, *args):
        self.cpu.mem_write(self.STACK, struct.pack(f'<{len(args) + 1}I', self.RETURN, *args))
        self.cpu.reg_write(UC_X86_REG_ESP, self.STACK)
        self.cpu.emu_start(address, self.RETURN, count=10000)
        if self.cpu.reg_read(UC_X86_REG_ESP) != self.STACK + 4:
            raise ValueError('original MIDI adapter did not complete')

    def capture(self, cpu, _address, _size, _data):
        stack = cpu.reg_read(UC_X86_REG_ESP)
        _, device, payload, length, timeout = struct.unpack('<5I', cpu.mem_read(stack, 20))
        if device != self.DEVICE or timeout != 5000:
            raise ValueError('unexpected original MIDI sink parameters')
        self.records.append('out ' + cpu.mem_read(payload, length).hex())
        cpu.reg_write(UC_X86_REG_EAX, 0)

    def transcript(self, source):
        self.records.clear()
        for line in source.splitlines():
            command, value = line.split(' ', 1)
            if command == 'chunk':
                if value != '256':
                    raise ValueError(f'unexpected original bridge chunk size {value}')
                self.call(self.SET_SIZE, self.ENGINE, int(value))
            elif command == 'sysex':
                message = bytes.fromhex(value)
                self.cpu.mem_write(self.PAYLOAD, message)
                # The original JNI path selects wait mode 0 for Line 6 hardware.
                self.call(self.SEND, self.ENGINE, self.PAYLOAD, len(message), 0)
            else:
                self.records.append(line)
        return '\n'.join(self.records) + '\n'

def verify(args, directory):
    if digest(args.archive.read_bytes()) != ARCHIVE_SHA256:
        raise ValueError('unexpected Variax 3.10 archive SHA-256')
    profile = subprocess.check_output([args.host, 'math-profile'], text=True).strip()
    if profile != 'host-std-v1':
        raise ValueError(f'expected host-std-v1, found {profile}')
    reference = directory / 'reference'
    subprocess.run([args.extractor, '--variax-reference', reference, args.archive],
                   stdout=subprocess.DEVNULL, check=True)
    presets = directory / 'presets'
    presets.mkdir()
    for slot in range(SLOTS):
        (presets / f'{slot}.vwm').write_bytes((reference / f'slot-{slot:02d}.vwm').read_bytes())

    oracle = CoefficientOracle(artifact_root=args.artifact_root)
    initial, manifest, _ = load_models(oracle, directory)

    def generate(settings, _mask):
        oracle.write(oracle.OBJECT, initial)
        return oracle.generate(settings)

    for mode in ('normal', 'muted'):
        phase = directory / mode
        phase.mkdir()
        original_programs(args.artifact_root, phase, presets, generate, muted=mode == 'muted')
        for slot in range(SLOTS):
            flags = ['muted'] if mode == 'muted' else []
            output = directory / f'rust-{slot}-{mode}.bin'
            subprocess.run([args.host, 'render-preset', presets / f'{slot}.vwm',
                            manifest, output, *flags], check=True)

    jdk = args.artifact_root / '.tools/jdk/bin'
    app = args.artifact_root / 'analysis/extracted/windows-app'
    classpath = os.pathsep.join(map(str, (directory / 'normal', app)))
    subprocess.run([jdk / 'javac', '-cp', classpath, '-d', directory / 'normal',
                    ROOT / 'scripts/VerifyVariaxWrite.java'], check=True)
    subprocess.run([jdk / 'java', '-cp', classpath, 'VerifyVariaxWrite', directory], check=True)
    chunker = OriginalChunker(args.artifact_root)
    for path in sorted(directory.glob('java-*.txt')):
        output = path.with_name(path.name.replace('java-', 'reference-', 1))
        output.write_text(chunker.transcript(path.read_text()))
    env = {**os.environ, 'VARIAX_WRITE_REFERENCE': str(directory)}
    subprocess.run(['cargo', 'test', '--offline', '--manifest-path',
                    'crates/line6-devices/Cargo.toml', 'original_transcripts',
                    '--', '--ignored'], cwd=ROOT, check=True, env=env)
    print('180 exact transcript comparisons (60 models × edit, store, and bank).')
    subprocess.run(['cargo', 'test', '--offline', '--lib', 'original_write_transcripts',
                    '--', '--ignored'], cwd=ROOT, check=True, env=env)
    print('180 transcript replays with explicit receiver-model gaps.')


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for option in ('artifact-root', 'archive', 'extractor', 'host'):
        parser.add_argument('--' + option, required=True, type=lambda value: Path(value).resolve())
    args = parser.parse_args()
    with tempfile.TemporaryDirectory(prefix='variax-write-') as directory:
        verify(args, Path(directory))


if __name__ == '__main__':
    main()
