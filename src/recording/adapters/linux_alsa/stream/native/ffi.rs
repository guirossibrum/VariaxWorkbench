//! ALSA 1.2.16.1 PCM header ABI. This is the only unsafe recorder module.
#![deny(unsafe_op_in_unsafe_fn)]

use std::ffi::{c_int, c_long, c_uint, c_ulong, c_void};
use std::ptr::{self, NonNull};

use libloading::{Error, Library};

use crate::recording::adapters::linux_alsa::AlsaNegotiatedPcm;
use crate::recording::capture::ValidatedPlan;

use super::{
    AlsaCaptureParams, AlsaCapturePcm, AlsaCaptureStream, AlsaStartError, CaptureAuthorization,
    Handle, NativeCalls, PcmAddress, PreparedPcmEvidence, start_prepared,
};

const TYPE_HW: c_int = 0;
const STREAM_CAPTURE: c_int = 1;
const STATE_PREPARED: c_int = 2;
const EIO: c_int = 5;
const EINVAL: c_int = 22;
const EOVERFLOW: c_int = 75;

#[repr(C)]
struct PcmInfo {
    _opaque: [u8; 0],
}

#[repr(C)]
struct HwParams {
    _opaque: [u8; 0],
}

type Pcm = c_void;

// Copy each symbol while retaining its defining library for the whole PCM
// lifetime. Signatures come from <alsa/pcm.h>.
macro_rules! pcm_symbols {
    ($(fn $name:ident($($arg:ty),*) -> $ret:ty;)*) => {
        pub struct Libasound {
            $($name: unsafe extern "C" fn($($arg),*) -> $ret,)*
            _library: Library,
        }

        impl Libasound {
            /// Loads the system libasound ABI without making a PCM call.
            ///
            /// # Safety
            /// The caller must trust the system dynamic loader and its library
            /// search path to resolve the genuine libasound ABI.
            pub unsafe fn load() -> Result<Self, Error> {
                // SAFETY: The caller vouches for the library search path.
                let library = unsafe { Library::new("libasound.so.2") }?;
                // SAFETY: Each symbol is typed to the pinned ALSA header ABI.
                Ok(unsafe {
                    Self {
                        $($name: *library.get(concat!(stringify!($name), "\0").as_bytes())?,)*
                        _library: library,
                    }
                })
            }
        }
    };
}

pcm_symbols! {
    fn snd_pcm_type(*mut Pcm) -> c_int;
    fn snd_pcm_stream(*mut Pcm) -> c_int;
    fn snd_pcm_state(*mut Pcm) -> c_int;
    fn snd_pcm_info_malloc(*mut *mut PcmInfo) -> c_int;
    fn snd_pcm_info_free(*mut PcmInfo) -> ();
    fn snd_pcm_info(*mut Pcm, *mut PcmInfo) -> c_int;
    fn snd_pcm_info_get_card(*const PcmInfo) -> c_int;
    fn snd_pcm_info_get_device(*const PcmInfo) -> c_uint;
    fn snd_pcm_info_get_subdevice(*const PcmInfo) -> c_uint;
    fn snd_pcm_info_get_stream(*const PcmInfo) -> c_int;
    fn snd_pcm_hw_params_malloc(*mut *mut HwParams) -> c_int;
    fn snd_pcm_hw_params_free(*mut HwParams) -> ();
    fn snd_pcm_hw_params_current(*mut Pcm, *mut HwParams) -> c_int;
    fn snd_pcm_hw_params_get_access(*const HwParams, *mut c_int) -> c_int;
    fn snd_pcm_hw_params_get_format(*const HwParams, *mut c_int) -> c_int;
    fn snd_pcm_hw_params_get_channels(*const HwParams, *mut c_uint) -> c_int;
    fn snd_pcm_hw_params_get_rate_numden(*const HwParams, *mut c_uint, *mut c_uint) -> c_int;
    fn snd_pcm_hw_params_get_period_size(*const HwParams, *mut c_ulong, *mut c_int) -> c_int;
    fn snd_pcm_hw_params_get_periods(*const HwParams, *mut c_uint, *mut c_int) -> c_int;
    fn snd_pcm_hw_params_get_buffer_size(*const HwParams, *mut c_ulong) -> c_int;
    fn snd_pcm_nonblock(*mut Pcm, c_int) -> c_int;
    fn snd_pcm_start(*mut Pcm) -> c_int;
    fn snd_pcm_readi(*mut Pcm, *mut c_void, c_ulong) -> c_long;
    fn snd_pcm_drop(*mut Pcm) -> c_int;
    fn snd_pcm_close(*mut Pcm) -> c_int;
}

impl Libasound {
    /// Takes ownership of an already prepared PCM for one capture attempt.
    /// The PCM is closed on admission or start failure.
    ///
    /// # Safety
    /// `handle` must be a live, exclusively owned `snd_pcm_t*` from this
    /// libasound instance, with no linked streams or concurrent users. The
    /// caller transfers close responsibility and must not use it afterwards.
    /// The caller must hold separate authority for this capture and provide
    /// independently verified evidence for this exact open attachment.
    pub unsafe fn adopt_prepared(
        self,
        _authorization: &CaptureAuthorization,
        handle: NonNull<c_void>,
        evidence: PreparedPcmEvidence,
        plan: ValidatedPlan,
        generation: u64,
    ) -> Result<AlsaCaptureStream<impl AlsaCapturePcm>, AlsaStartError> {
        start_prepared(self, handle, evidence, plan, generation)
    }
}

impl NativeCalls for Libasound {
    fn prepared_capture_address(&self, handle: Handle) -> Option<PcmAddress> {
        let pcm = handle.as_ptr();
        // SAFETY: The handle was transferred exclusively by adopt_prepared.
        // ALSA initializes the info object before any getter is called.
        unsafe {
            if (self.snd_pcm_type)(pcm) != TYPE_HW
                || (self.snd_pcm_stream)(pcm) != STREAM_CAPTURE
                || (self.snd_pcm_state)(pcm) != STATE_PREPARED
            {
                return None;
            }
            let info = AlsaObject::new(self.snd_pcm_info_malloc, self.snd_pcm_info_free).ok()?;
            let info = info.ptr.as_ptr();
            checked((self.snd_pcm_info)(pcm, info)).ok()?;
            if (self.snd_pcm_info_get_stream)(info) != STREAM_CAPTURE {
                return None;
            }
            Some(PcmAddress {
                card: (self.snd_pcm_info_get_card)(info).try_into().ok()?,
                device: (self.snd_pcm_info_get_device)(info),
                subdevice: (self.snd_pcm_info_get_subdevice)(info),
            })
        }
    }

    fn current_params(&self, handle: Handle) -> Result<AlsaCaptureParams, i32> {
        // SAFETY: The PCM is live and exclusive. All out-parameters have
        // their header-declared types and the params object is ALSA-owned.
        unsafe {
            let params =
                AlsaObject::new(self.snd_pcm_hw_params_malloc, self.snd_pcm_hw_params_free)?;
            let params = params.ptr.as_ptr();
            checked((self.snd_pcm_hw_params_current)(handle.as_ptr(), params))?;
            let mut access = 0;
            let mut format = 0;
            let mut channels = 0;
            let mut rate_num = 0;
            let mut rate_den = 0;
            let mut period_frames = 0;
            let mut period_direction = 0;
            let mut periods = 0;
            let mut periods_direction = 0;
            let mut buffer_frames = 0;
            checked((self.snd_pcm_hw_params_get_access)(params, &mut access))?;
            checked((self.snd_pcm_hw_params_get_format)(params, &mut format))?;
            checked((self.snd_pcm_hw_params_get_channels)(params, &mut channels))?;
            checked((self.snd_pcm_hw_params_get_rate_numden)(
                params,
                &mut rate_num,
                &mut rate_den,
            ))?;
            checked((self.snd_pcm_hw_params_get_period_size)(
                params,
                &mut period_frames,
                &mut period_direction,
            ))?;
            checked((self.snd_pcm_hw_params_get_periods)(
                params,
                &mut periods,
                &mut periods_direction,
            ))?;
            checked((self.snd_pcm_hw_params_get_buffer_size)(
                params,
                &mut buffer_frames,
            ))?;
            if period_direction != 0 || periods_direction != 0 {
                return Err(-EINVAL);
            }
            Ok(AlsaCaptureParams {
                pcm: AlsaNegotiatedPcm {
                    access,
                    format,
                    channels,
                    rate_num,
                    rate_den,
                },
                period_frames: narrow_frames(period_frames)?,
                periods,
                buffer_frames: narrow_frames(buffer_frames)?,
            })
        }
    }

    fn nonblock(&self, handle: Handle) -> Result<(), i32> {
        // SAFETY: The handle is live and exclusive.
        checked(unsafe { (self.snd_pcm_nonblock)(handle.as_ptr(), 1) })
    }

    fn start(&self, handle: Handle) -> Result<(), i32> {
        // SAFETY: The handle is live and exclusive.
        checked(unsafe { (self.snd_pcm_start)(handle.as_ptr()) })
    }

    fn readi(&self, handle: Handle, frames: usize, buffer: &mut [u8]) -> isize {
        if frames.checked_mul(crate::pod_audio::FRAME_BYTES as usize) != Some(buffer.len()) {
            return -(EINVAL as isize);
        }
        // SAFETY: Admission verified the exclusive PCM's six-byte frame
        // format. The buffer is writable and bounded for the requested frames.
        unsafe {
            (self.snd_pcm_readi)(
                handle.as_ptr(),
                buffer.as_mut_ptr().cast(),
                frames as c_ulong,
            ) as isize
        }
    }

    fn drop_pcm(&self, handle: Handle) -> Result<(), i32> {
        // SAFETY: The handle is live and exclusive.
        checked(unsafe { (self.snd_pcm_drop)(handle.as_ptr()) })
    }

    fn close(&self, handle: Handle) -> Result<(), i32> {
        // SAFETY: NativePcm takes the handle out of its Option before closing.
        checked(unsafe { (self.snd_pcm_close)(handle.as_ptr()) })
    }
}

fn checked(result: c_int) -> Result<(), i32> {
    if result < 0 { Err(result) } else { Ok(()) }
}

fn narrow_frames(frames: c_ulong) -> Result<u32, i32> {
    u32::try_from(frames).map_err(|_| -EOVERFLOW)
}

struct AlsaObject<T> {
    ptr: NonNull<T>,
    free: unsafe extern "C" fn(*mut T),
}

impl<T> AlsaObject<T> {
    unsafe fn new(
        allocate: unsafe extern "C" fn(*mut *mut T) -> c_int,
        free: unsafe extern "C" fn(*mut T),
    ) -> Result<Self, i32> {
        let mut object = ptr::null_mut();
        // SAFETY: This constructor receives matching ALSA allocation functions.
        checked(unsafe { allocate(&mut object) })?;
        Ok(Self {
            ptr: NonNull::new(object).ok_or(-EIO)?,
            free,
        })
    }
}

impl<T> Drop for AlsaObject<T> {
    fn drop(&mut self) {
        // SAFETY: This is the unique object from the matching ALSA allocation.
        unsafe { (self.free)(self.ptr.as_ptr()) };
    }
}
