cfg_if! {
    if #[cfg(all(target_arch = "wasm32", feature = "wbindgen"))] {
        mod wasm_bindgen;
        pub use self::wasm_bindgen::random::Random;
        pub use self::wasm_bindgen::instant::Instant;
    }
    else if #[cfg(all(target_arch = "wasm32", feature = "mquad"))] {
        mod miniquad;
        pub use self::miniquad::random::Random;
        pub use self::miniquad::instant::Instant;
    }
    else if #[cfg(all(not(target_arch = "wasm32"), feature = "webrtc-full"))] {
        mod webrtc;
        pub use self::webrtc::random::Random;
        pub use self::webrtc::instant::Instant;
        pub use self::webrtc::rtc_peer;
    }
    // else if #[cfg(all(not(target_arch = "wasm32"), feature = "webrtc-lite"))] {
    else {
        mod native;
        pub use self::native::random::Random;
        pub use self::native::instant::Instant;
    }
    // else {
    //     compile_error!("Naia Socket Shared on Native requires either the 'webrtc-lite' OR 'webrtc-full' feature to be enabled, you must pick one.");
    // }
}
