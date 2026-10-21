//! A simulator's screen as its helper streams it: H.264 frames, each a run of
//! units after start codes, decoded by VideoToolbox into pictures GPUI draws
//! with `surface`, on the GPU and without copying them.

use std::ffi::c_void;
use std::ptr::{self, NonNull};
use std::sync::{Arc, Mutex};

use core_foundation::base::{CFType, TCFType};
use core_foundation::boolean::CFBoolean;
use core_foundation::dictionary::CFDictionary;
use core_foundation::number::CFNumber;
use core_foundation::string::{CFString, CFStringRef};
use objc2_core_foundation::CFRetained;
use objc2_core_video::CVPixelBuffer;
use sikemux_process::sync::Locked;

const SPS: u8 = 7;
const PPS: u8 = 8;
const KEY_FRAME: u8 = 5;
/// GPUI's surfaces take bi-planar 4:2:0, full range, as its Metal renderer reads them.
const PIXEL_FORMAT: i32 = i32::from_be_bytes(*b"420f");

type OsStatus = i32;

#[repr(C)]
#[derive(Clone, Copy)]
struct CmTime {
    value: i64,
    timescale: i32,
    flags: u32,
    epoch: i64,
}

#[repr(C)]
struct CmSampleTimingInfo {
    duration: CmTime,
    presentation: CmTime,
    decode: CmTime,
}

type Decoded = extern "C" fn(
    refcon: *mut c_void,
    frame_refcon: *mut c_void,
    status: OsStatus,
    flags: u32,
    image: *mut c_void,
    presentation: CmTime,
    duration: CmTime,
);

#[repr(C)]
struct DecompressionCallback {
    decoded: Decoded,
    refcon: *mut c_void,
}

#[link(name = "CoreMedia", kind = "framework")]
extern "C" {
    static kCMTimeInvalid: CmTime;
    fn CMVideoFormatDescriptionCreateFromH264ParameterSets(
        allocator: *const c_void,
        count: usize,
        pointers: *const *const u8,
        sizes: *const usize,
        unit_header_length: i32,
        out: *mut *const c_void,
    ) -> OsStatus;
    fn CMBlockBufferCreateWithMemoryBlock(
        allocator: *const c_void,
        memory: *mut c_void,
        length: usize,
        block_allocator: *const c_void,
        custom_source: *const c_void,
        offset: usize,
        data_length: usize,
        flags: u32,
        out: *mut *mut c_void,
    ) -> OsStatus;
    fn CMBlockBufferReplaceDataBytes(
        source: *const c_void,
        destination: *mut c_void,
        offset: usize,
        length: usize,
    ) -> OsStatus;
    fn CMSampleBufferCreateReady(
        allocator: *const c_void,
        data: *mut c_void,
        format: *const c_void,
        samples: isize,
        timings: isize,
        timing: *const CmSampleTimingInfo,
        sizes: isize,
        size: *const usize,
        out: *mut *mut c_void,
    ) -> OsStatus;
}

#[link(name = "VideoToolbox", kind = "framework")]
extern "C" {
    fn VTDecompressionSessionCreate(
        allocator: *const c_void,
        format: *const c_void,
        decoder: *const c_void,
        image_attributes: *const c_void,
        callback: *const DecompressionCallback,
        out: *mut *mut c_void,
    ) -> OsStatus;
    fn VTDecompressionSessionDecodeFrame(
        session: *mut c_void,
        sample: *mut c_void,
        flags: u32,
        frame_refcon: *mut c_void,
        info: *mut u32,
    ) -> OsStatus;
    fn VTDecompressionSessionInvalidate(session: *mut c_void);
}

#[link(name = "CoreVideo", kind = "framework")]
extern "C" {
    static kCVPixelBufferPixelFormatTypeKey: CFStringRef;
    static kCVPixelBufferIOSurfacePropertiesKey: CFStringRef;
    static kCVPixelBufferMetalCompatibilityKey: CFStringRef;
}

#[link(name = "CoreFoundation", kind = "framework")]
extern "C" {
    fn CFRetain(object: *const c_void) -> *const c_void;
    fn CFRelease(object: *const c_void);
}

/// The newest picture the decoder made, which the window draws next.
pub type Picture = Arc<Mutex<Option<Frame>>>;

/// One decoded picture, held for the window's thread.
pub struct Frame(NonNull<CVPixelBuffer>);

// SAFETY: a pixel buffer is a reference-counted Core Video object, and its reference
// count and IOSurface are safe to use from any thread; the window only reads it.
unsafe impl Send for Frame {}

impl Frame {
    /// The picture, as GPUI's `surface` draws it.
    pub fn buffer(&self) -> CFRetained<CVPixelBuffer> {
        // SAFETY: `self` holds a reference, so the buffer is alive.
        unsafe { CFRetained::retain(self.0) }
    }
}

impl Drop for Frame {
    fn drop(&mut self) {
        // SAFETY: releases the reference `decoded` took.
        unsafe { CFRelease(self.0.as_ptr().cast_const().cast()) };
    }
}

pub struct Decoder {
    sps: Vec<u8>,
    pps: Vec<u8>,
    session: Option<Session>,
    picture: Picture,
}

// SAFETY: the session is used from one thread at a time, as the decoder is moved, never
// shared, and its callback only writes to the `Picture` behind its mutex.
unsafe impl Send for Decoder {}

/// A decompression session and the format it was made for, released together.
struct Session {
    session: *mut c_void,
    format: *const c_void,
    /// The callback's own pointer to where pictures go, freed with the session.
    picture: *mut Picture,
}

impl Drop for Session {
    fn drop(&mut self) {
        // SAFETY: both were created by `Decoder::start` and are released once, here; the
        // session is invalidated first, so its callback never runs after its box goes.
        unsafe {
            VTDecompressionSessionInvalidate(self.session);
            CFRelease(self.session.cast_const());
            CFRelease(self.format);
            drop(Box::from_raw(self.picture));
        }
    }
}

impl Decoder {
    pub fn new(picture: Picture) -> Self {
        Self {
            sps: Vec::new(),
            pps: Vec::new(),
            session: None,
            picture,
        }
    }

    /// Decodes one frame from the helper. A frame with new parameter sets starts a
    /// new session; frames before the first key frame are skipped.
    pub fn decode(&mut self, frame: &[u8]) -> Result<(), String> {
        let units = split_units(frame);
        let mut changed = false;
        for unit in &units {
            match unit.first().map(|header| header & 0x1f) {
                Some(SPS) if self.sps != *unit => {
                    self.sps = unit.to_vec();
                    changed = true;
                }
                Some(PPS) if self.pps != *unit => {
                    self.pps = unit.to_vec();
                    changed = true;
                }
                _ => {}
            }
        }
        if changed || self.session.is_none() {
            let key = units
                .iter()
                .any(|unit| unit.first().map(|header| header & 0x1f) == Some(KEY_FRAME));
            if !key || self.sps.is_empty() || self.pps.is_empty() {
                return Ok(());
            }
            self.session = None;
            self.session = Some(self.start()?);
        }
        let picture: Vec<u8> = units
            .iter()
            .filter(|unit| !matches!(unit.first().map(|header| header & 0x1f), Some(SPS | PPS)))
            .flat_map(|unit| {
                (unit.len() as u32)
                    .to_be_bytes()
                    .into_iter()
                    .chain(unit.iter().copied())
            })
            .collect();
        if picture.is_empty() {
            return Ok(());
        }
        let session = self.session.as_ref().expect("a session was just made");
        // SAFETY: the block buffer allocates and owns its own copy of `picture`; the
        // sample buffer and block buffer are released below after a synchronous decode.
        unsafe {
            let mut block = ptr::null_mut();
            let status = CMBlockBufferCreateWithMemoryBlock(
                ptr::null(),
                ptr::null_mut(),
                picture.len(),
                ptr::null(),
                ptr::null(),
                0,
                picture.len(),
                0,
                &mut block,
            );
            if status != 0 || block.is_null() {
                return Err(format!(
                    "the screen's frame could not be wrapped ({status})"
                ));
            }
            let status =
                CMBlockBufferReplaceDataBytes(picture.as_ptr().cast(), block, 0, picture.len());
            if status != 0 {
                CFRelease(block.cast_const());
                return Err(format!("the screen's frame could not be copied ({status})"));
            }
            let timing = CmSampleTimingInfo {
                duration: kCMTimeInvalid,
                presentation: kCMTimeInvalid,
                decode: kCMTimeInvalid,
            };
            let size = picture.len();
            let mut sample = ptr::null_mut();
            let status = CMSampleBufferCreateReady(
                ptr::null(),
                block,
                session.format,
                1,
                1,
                &timing,
                1,
                &size,
                &mut sample,
            );
            CFRelease(block.cast_const());
            if status != 0 || sample.is_null() {
                return Err(format!(
                    "the screen's frame could not be sampled ({status})"
                ));
            }
            let mut info = 0;
            let status = VTDecompressionSessionDecodeFrame(
                session.session,
                sample,
                0,
                ptr::null_mut(),
                &mut info,
            );
            CFRelease(sample.cast_const());
            if status != 0 {
                self.session = None;
                return Err(format!(
                    "the screen's frame could not be decoded ({status})"
                ));
            }
        }
        Ok(())
    }

    fn start(&self) -> Result<Session, String> {
        let pointers = [self.sps.as_ptr(), self.pps.as_ptr()];
        let sizes = [self.sps.len(), self.pps.len()];
        let mut format = ptr::null();
        // SAFETY: two parameter sets with their lengths, read during the call only.
        let status = unsafe {
            CMVideoFormatDescriptionCreateFromH264ParameterSets(
                ptr::null(),
                2,
                pointers.as_ptr(),
                sizes.as_ptr(),
                4,
                &mut format,
            )
        };
        if status != 0 || format.is_null() {
            return Err(format!("the screen's stream could not be read ({status})"));
        }
        // SAFETY: CoreVideo's keys are immortal constants, borrowed here.
        let (pixel_format, surface, metal) = unsafe {
            (
                CFString::wrap_under_get_rule(kCVPixelBufferPixelFormatTypeKey),
                CFString::wrap_under_get_rule(kCVPixelBufferIOSurfacePropertiesKey),
                CFString::wrap_under_get_rule(kCVPixelBufferMetalCompatibilityKey),
            )
        };
        let none: CFDictionary<CFString, CFType> = CFDictionary::from_CFType_pairs(&[]);
        let attributes: CFDictionary<CFString, CFType> = CFDictionary::from_CFType_pairs(&[
            (pixel_format, CFNumber::from(PIXEL_FORMAT).as_CFType()),
            (surface, none.as_CFType()),
            (metal, CFBoolean::true_value().as_CFType()),
        ]);
        let picture = Box::into_raw(Box::new(Arc::clone(&self.picture)));
        let callback = DecompressionCallback {
            decoded,
            refcon: picture.cast(),
        };
        let mut session = ptr::null_mut();
        // SAFETY: `format` and the attributes are live; the session copies the callback
        // record, and its refcon box lives until `Session` drops.
        let status = unsafe {
            VTDecompressionSessionCreate(
                ptr::null(),
                format,
                ptr::null(),
                attributes.as_concrete_TypeRef().cast(),
                &callback,
                &mut session,
            )
        };
        if status != 0 || session.is_null() {
            // SAFETY: made above and not handed to anything else.
            unsafe {
                CFRelease(format);
                drop(Box::from_raw(picture));
            }
            return Err(format!(
                "the screen's stream could not be decoded ({status})"
            ));
        }
        Ok(Session {
            session,
            format,
            picture,
        })
    }
}

extern "C" fn decoded(
    refcon: *mut c_void,
    _frame_refcon: *mut c_void,
    status: OsStatus,
    _flags: u32,
    image: *mut c_void,
    _presentation: CmTime,
    _duration: CmTime,
) {
    let Some(image) = NonNull::new(image.cast::<CVPixelBuffer>()).filter(|_| status == 0) else {
        return;
    };
    // SAFETY: `refcon` is the session's own `Picture` box, alive while the session is,
    // and VideoToolbox lends a live pixel buffer for the length of the call, which is retained.
    unsafe {
        CFRetain(image.as_ptr().cast_const().cast());
        let picture = &*refcon.cast::<Picture>();
        *picture.locked() = Some(Frame(image));
    }
}

/// The units of one frame, each after a 00 00 01 or 00 00 00 01 start code.
fn split_units(frame: &[u8]) -> Vec<&[u8]> {
    let mut starts = Vec::new();
    let mut index = 0;
    while index + 2 < frame.len() {
        if frame[index] == 0 && frame[index + 1] == 0 {
            if frame[index + 2] == 1 {
                starts.push((index, 3));
                index += 3;
                continue;
            }
            if index + 3 < frame.len() && frame[index + 2] == 0 && frame[index + 3] == 1 {
                starts.push((index, 4));
                index += 4;
                continue;
            }
        }
        index += 1;
    }
    starts
        .iter()
        .enumerate()
        .map(|(at, &(start, length))| {
            let end = starts.get(at + 1).map_or(frame.len(), |&(next, _)| next);
            &frame[start + length..end]
        })
        .collect()
}
