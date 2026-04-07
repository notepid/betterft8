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

    // Link the Hamlib library when the feature is enabled.
    // On Windows: auto-extracts hamlib-w64.zip if hamlib.lib is missing.
    // On Linux: expects system-installed libhamlib-dev.
    #[cfg(feature = "hamlib")]
    {
        let hamlib_dir = std::path::Path::new("hamlib-lib");

        // Auto-extract the vendored Windows zip if the import lib is missing.
        if cfg!(target_os = "windows") {
            let lib_file = hamlib_dir.join("hamlib.lib");
            let zip_file = hamlib_dir.join("hamlib-w64.zip");
            if !lib_file.exists() && zip_file.exists() {
                println!("cargo:warning=Extracting hamlib-w64.zip...");
                let file = std::fs::File::open(&zip_file)
                    .expect("failed to open hamlib-w64.zip");
                let mut archive = zip::ZipArchive::new(file)
                    .expect("failed to read hamlib-w64.zip");
                archive.extract(hamlib_dir)
                    .expect("failed to extract hamlib-w64.zip");
            }
        }

        let hamlib_dir = hamlib_dir
            .canonicalize()
            .expect("hamlib-lib/ directory not found — see README for setup instructions");
        println!("cargo:rustc-link-search=native={}", hamlib_dir.display());
        println!("cargo:rustc-link-lib=dylib=hamlib");
        if cfg!(target_os = "windows") {
            println!("cargo:rustc-link-lib=ws2_32");
        }
        println!("cargo:rerun-if-changed=hamlib-lib/hamlib-w64.zip");
    }
}
