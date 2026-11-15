"""Copy recovered raster assets unchanged and build a local visual catalog."""
import hashlib
import html
import json
from pathlib import Path
import shutil

from PIL import Image, ImageDraw, ImageFont

ROOT = Path(__file__).resolve().parents[1]
SOURCE = ROOT / "analysis/extracted/windows-app"
OUTPUT = ROOT / "assets/original-ui"
REPORT = ROOT / "analysis/ui-assets"


def sheet(files, title, target, columns=5, cell=(200, 190)):
    rows = (len(files) + columns - 1) // columns
    result = Image.new("RGB", (columns * cell[0], rows * cell[1] + 48), "#202329")
    draw = ImageDraw.Draw(result)
    draw.text((16, 14), title, fill="white", font=ImageFont.load_default(size=18))
    for i, path in enumerate(files):
        x, y = (i % columns) * cell[0], (i // columns) * cell[1] + 48
        draw.rectangle((x+4, y+4, x+cell[0]-4, y+cell[1]-4), fill="#333840")
        with Image.open(path) as original:
            preview = original.convert("RGBA")
            preview.thumbnail((cell[0]-20, cell[1]-40))
            result.paste(preview, (x+(cell[0]-preview.width)//2, y+8), preview)
        draw.text((x+10, y+cell[1]-24), path.name, fill="white")
    result.save(target)


def main():
    OUTPUT.mkdir(parents=True, exist_ok=True)
    REPORT.mkdir(parents=True, exist_ok=True)
    records = []
    for path in sorted(SOURCE.rglob("*")):
        if not path.is_file() or path.suffix.lower() not in (".png", ".gif", ".jpg", ".jpeg", ".bmp", ".ico"):
            continue
        relative = path.relative_to(SOURCE)
        target = OUTPUT / relative
        target.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(path, target)
        with Image.open(path) as original:
            records.append({"path": str(relative), "width": original.width,
                            "height": original.height, "format": original.format,
                            "mode": original.mode, "frames": getattr(original, "n_frames", 1),
                            "bytes": path.stat().st_size,
                            "sha256": hashlib.sha256(path.read_bytes()).hexdigest()})
    (REPORT / "manifest.json").write_text(json.dumps(records, indent=2) + "\n")
    electric = OUTPUT / "modules/variax/editor/images"
    sheet(sorted((electric / "bodies").glob("*.png")), "Original Workbench guitar body artwork", REPORT / "guitar-bodies.png", cell=(200, 170))
    sheet(sorted((electric / "pickups").glob("*.png")), "Original Workbench pickup artwork", REPORT / "pickups.png", columns=4, cell=(220, 130))
    neck = sorted(p for p in electric.glob("*.png") if any(s in p.name for s in ("fret", "neck", "head", "string")))
    if neck:
        sheet(neck, "Original neck / tuning graphics", REPORT / "neck-and-tuning.png", columns=2, cell=(450, 220))
    cards = []
    for r in records:
        url = "../../assets/original-ui/" + r["path"]
        cards.append(f'<figure data-path="{html.escape(r["path"], quote=True)}"><div><img loading="lazy" src="{html.escape(url, quote=True)}"></div><figcaption>{html.escape(r["path"])}<br>{r["width"]} × {r["height"]}, {r["frames"]} frame(s)</figcaption></figure>')
    document = '<!doctype html><meta charset="utf-8"><title>Original Workbench image catalog</title><style>body{background:#202329;color:#eee;font:14px system-ui;margin:24px}input{padding:12px;width:70%;margin:10px 0 24px}main{display:grid;grid-template-columns:repeat(auto-fill,minmax(260px,1fr));gap:16px}figure{margin:0;background:#333840;padding:12px;overflow-wrap:anywhere}figure div{height:180px;display:flex;align-items:center;justify-content:center}img{max-height:180px;max-width:100%;object-fit:contain}figcaption{margin-top:12px;font-size:12px}</style><h1>Original Workbench image catalog</h1><p>Unmodified extracted assets. Sprite sheets contain multiple control states; these are not live UI screenshots.</p><input id="filter" placeholder="Filter: pickups, bodies, fretboard, controls, help…"><main>' + "\n".join(cards) + '</main><script>document.getElementById("filter").addEventListener("input",e=>{const q=e.target.value.toLowerCase();for(const f of document.querySelectorAll("figure"))f.hidden=!f.dataset.path.toLowerCase().includes(q)})</script>'
    (REPORT / "index.html").write_text(document)
    print(f"Copied {len(records)} original raster assets; catalog: {REPORT / 'index.html'}")
    print(f"Electric editor: {sum(r['path'].startswith('modules/variax/editor/images/') for r in records)} assets")


if __name__ == "__main__":
    main()
