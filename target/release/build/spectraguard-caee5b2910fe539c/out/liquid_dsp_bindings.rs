pub type fftplan = *mut ::libc::c_void;

pub const LIQUID_FFT_FORWARD: i32 = 0;
pub const LIQUID_FFT_BACKWARD: i32 = 1;

pub unsafe fn fft_create_plan(
    _nfft: u32,
    _input: *mut ::libc::c_void,
    _output: *mut ::libc::c_void,
    _direction: i32,
    _method: i32,
) -> fftplan {
    ::core::ptr::null_mut()
}

pub unsafe fn fft_execute(_plan: fftplan) -> i32 {
    0
}

pub unsafe fn fft_destroy_plan(_plan: fftplan) -> i32 {
    0
}
