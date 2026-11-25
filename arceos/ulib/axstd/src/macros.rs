//! Standard library macros

/// Prints to the standard output.
///
/// Equivalent to the [`println!`] macro except that a newline is not printed at
/// the end of the message.
///
/// [`println!`]: crate::println
#[macro_export]
macro_rules! print {
    ($($arg:tt)*) => {
        // ****** START xisanlou add at  25102404 No.1
        //$crate::io::__print_impl(format_args!($($arg)*));
        $crate::io::__print_impl(format_args!("\x1b[95m{}\x1b[0m", format_args!($($arg)*)));
        // ****** END   xisanlou add at 25102404 No.1
    }
}

/// Prints to the standard output, with a newline.
#[macro_export]
macro_rules! println {
    () => { $crate::print!("\n") };
    ($($arg:tt)*) => {
        // ****** START xisanlou add at  25102404 No.2
        //$crate::io::__print_impl(format_args!("{}\n", format_args!($($arg)*)));
        $crate::io::__print_impl(format_args!("\x1b[95m{}\x1b[0m\n", format_args!($($arg)*)));
        // ****** END   xisanlou add at 25102404 No.2
    }
}
