//! AppKit main-thread integration test harness without libtest.

fn main() {
    #[cfg(all(target_os = "macos", debug_assertions))]
    oink_clipboard::run_native_linked_promise_tests();
}
