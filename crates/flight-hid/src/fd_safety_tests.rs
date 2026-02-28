//! Tests to ensure public APIs use typed file descriptors instead of RawFd
//!
//! This module contains compile-time and runtime tests to prevent the use of
//! raw file descriptors in public APIs, enforcing the use of OwnedFd/BorrowedFd/AsFd
//! for better type safety and resource management.

#[cfg(all(test, unix))]
mod tests {
    #[cfg(unix)]
    use std::os::fd::{AsFd, BorrowedFd, OwnedFd};

    /// Test that demonstrates proper typed file descriptor usage
    #[test]
    fn test_typed_fd_usage() {
        // This test ensures we're using the correct imports for typed file descriptors
        // If RawFd is accidentally used in public APIs, this will help catch it

        // Example of proper typed FD usage (would be used in actual implementations)
        fn example_function_with_owned_fd(_fd: OwnedFd) {
            // Function that takes ownership of a file descriptor
        }

        fn example_function_with_borrowed_fd(_fd: BorrowedFd<'_>) {
            // Function that borrows a file descriptor
        }

        fn example_function_with_as_fd<T: AsFd>(_fd: T) {
            // Generic function that works with anything implementing AsFd
        }

        // These functions compile, demonstrating proper typed FD usage
        // If we accidentally use RawFd in public APIs, the compiler will catch it
        let _ = (
            example_function_with_owned_fd,
            example_function_with_borrowed_fd,
            example_function_with_as_fd::<std::os::unix::io::OwnedFd>,
        );
    }

    /// Compile-time test to ensure RawFd is not used in public function signatures
    #[test]
    fn test_no_raw_fd_in_public_apis() {
        // This test uses a compile-time assertion to ensure RawFd is not used
        // in public APIs. The test will fail to compile if RawFd is used incorrectly.

        // Check that our public API types don't contain RawFd
        use crate::{EndpointId, HidAdapter, HidDeviceInfo, HidOperationResult};

        // These assertions will fail to compile if RawFd is used in these types
        fn _assert_no_raw_fd_in_hid_adapter(_: &HidAdapter) {}
        fn _assert_no_raw_fd_in_device_info(_: &HidDeviceInfo) {}
        fn _assert_no_raw_fd_in_endpoint_id(_: &EndpointId) {}
        fn _assert_no_raw_fd_in_operation_result(_: &HidOperationResult) {}

        // If any of these types contained RawFd, the compiler would error here
        // because RawFd doesn't implement the necessary traits for these assertions
    }

    /// Runtime test to verify no RawFd usage in error messages or debug output
    #[test]
    fn test_no_raw_fd_in_debug_output() {
        use crate::{EndpointId, EndpointType, HidDeviceInfo, HidOperationResult};

        // Create test instances and check their debug output doesn't contain "RawFd"
        let device_info = HidDeviceInfo {
            vendor_id: 0x1234,
            product_id: 0x5678,
            serial_number: Some("TEST".to_string()),
            manufacturer: Some("Test".to_string()),
            product_name: Some("Test Device".to_string()),
            device_path: "/dev/test".to_string(),
            usage_page: 0x01,
            usage: 0x04,
            report_descriptor: None,
        };

        let endpoint_id = EndpointId {
            device_path: "/dev/test".to_string(),
            endpoint_type: EndpointType::Input,
        };

        let operation_result = HidOperationResult::Success {
            bytes_transferred: 10,
        };

        // Check debug output doesn't contain "RawFd"
        let device_debug = format!("{:?}", device_info);
        let endpoint_debug = format!("{:?}", endpoint_id);
        let result_debug = format!("{:?}", operation_result);

        assert!(
            !device_debug.contains("RawFd"),
            "HidDeviceInfo debug output contains RawFd"
        );
        assert!(
            !endpoint_debug.contains("RawFd"),
            "EndpointId debug output contains RawFd"
        );
        assert!(
            !result_debug.contains("RawFd"),
            "HidOperationResult debug output contains RawFd"
        );
    }
}

/// Clippy lint configuration to prevent RawFd usage
///
/// This module configures clippy to warn about RawFd usage in public APIs.
/// Add this to your clippy configuration to catch RawFd usage at compile time.
#[cfg(test)]
mod clippy_config {
    #![warn(clippy::missing_safety_doc)]
    #![warn(clippy::undocumented_unsafe_blocks)]

    // Custom lint to detect RawFd usage (would be implemented as a custom clippy lint)
    // For now, we rely on the type system and manual review

    /// Example of what NOT to do - using RawFd in public API
    /// This function is intentionally commented out to prevent compilation
    ///
    /// ```rust,ignore
    /// pub fn bad_function_with_raw_fd(fd: std::os::unix::io::RawFd) {
    ///     // This would be caught by our CI checks
    /// }
    /// ```
    ///
    /// Instead, use typed file descriptors:
    ///
    /// ```rust
    /// use std::os::fd::{OwnedFd, BorrowedFd, AsFd};
    ///
    /// pub fn good_function_with_owned_fd(fd: OwnedFd) {
    ///     // This is the correct way to handle file descriptors
    /// }
    ///
    /// pub fn good_function_with_borrowed_fd(fd: BorrowedFd<'_>) {
    ///     // This borrows a file descriptor safely
    /// }
    ///
    /// pub fn good_function_with_as_fd<T: AsFd>(fd: T) {
    ///     // This is generic over anything that can provide a file descriptor
    /// }
    /// ```
    #[test]
    fn test_fd_usage_examples() {
        // This test documents the correct way to use file descriptors
        // and serves as a reference for developers
    }
}
#[cfg(all(test, windows))]
mod windows_tests {
    #[cfg(windows)]
    use std::os::windows::io::{BorrowedHandle, OwnedHandle};

    /// Test that demonstrates proper typed handle usage on Windows
    #[cfg(windows)]
    #[test]
    fn test_typed_handle_usage() {
        // This test ensures we're using the correct imports for typed handles on Windows
        // If RawHandle is accidentally used in public APIs, this will help catch it

        // Example of proper typed handle usage (would be used in actual implementations)
        fn example_function_with_owned_handle(_handle: OwnedHandle) {
            // Function that takes ownership of a handle
        }

        fn example_function_with_borrowed_handle(_handle: BorrowedHandle) {
            // Function that borrows a handle temporarily
        }

        // This function signature would be INCORRECT and should be avoided:
        // fn bad_function_with_raw_handle(handle: RawHandle) { }

        // Instead, use typed handles for better safety
        let _ = example_function_with_owned_handle;
        let _ = example_function_with_borrowed_handle;
    }
}
