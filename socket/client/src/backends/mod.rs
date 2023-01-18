cfg_if! {
    if #[cfg(all(target_arch = "wasm32", feature = "wbindgen"))] {
        mod wasm_bindgen;
        pub use self::wasm_bindgen::*;
    }
    else if #[cfg(all(target_arch = "wasm32", feature = "mquad"))] {
        mod miniquad;
        pub use self::miniquad::*;
    }
    else if #[cfg(all(not(target_arch = "wasm32"), feature = "webrtc-lite"))] {
        mod native;
        pub use self::native::*;
    }
    else if #[cfg(all(not(target_arch = "wasm32"), feature = "webrtc-full"))] {
        mod webrtc;
        pub use self::webrtc::*;
    }
}
