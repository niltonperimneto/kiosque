//! `quick_test_main` from Qt Quick Test. cxx generates the C++ shim; there
//! is no hand-written C++ in the project.

#[cxx::bridge]
pub mod ffi {
    unsafe extern "C++" {
        include!("QtQuickTest/quicktest.h");

        /// Runs every `tst_*.qml` below `source_dir` (or the `-input` given in
        /// `argv`) and returns the number of failures.
        ///
        /// # Safety
        /// `argv` must hold `argc` valid C strings; all pointers must outlive the call.
        unsafe fn quick_test_main(
            argc: i32,
            argv: *mut *mut c_char,
            name: *const c_char,
            source_dir: *const c_char,
        ) -> i32;
    }
}
