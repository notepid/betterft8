fn main() {
    let mut build = cc::Build::new();

    build
        .files([
            "ft8_lib/ft8/decode.c",
            "ft8_lib/ft8/encode.c",
            "ft8_lib/ft8/ldpc.c",
            "ft8_lib/ft8/constants.c",
            "ft8_lib/ft8/crc.c",
            "ft8_lib/ft8/message.c",
            "ft8_lib/ft8/text.c",
            "ft8_lib/common/monitor.c",
            "ft8_lib/fft/kiss_fft.c",
            "ft8_lib/fft/kiss_fftr.c",
            "ft8_wrapper.c",
        ])
        // ft8_lib headers are referenced as <ft8/...>, <common/...>, <fft/...>
        .include("ft8_lib")
        // Wrapper header and msvc_compat.h live here
        .include(".");

    // MSVC doesn't provide stpcpy; force-include our shim for all C files
    if build.get_compiler().is_like_msvc() {
        build.flag("/FImsvc_compat.h");
    }

    build.compile("ft8");

    println!("cargo:rerun-if-changed=ft8_lib/");
    println!("cargo:rerun-if-changed=ft8_wrapper.c");
    println!("cargo:rerun-if-changed=ft8_wrapper.h");
}
