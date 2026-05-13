use core::fmt::{self, Write as _};

use heapless::String;

pub(crate) fn tag_for_module(module: &str) -> &'static str {
    if module == "tbox::can" || module.contains("::can") {
        "can"
    } else if module == "tbox::modem" || module.contains("::modem") {
        "modem"
    } else {
        "kernel"
    }
}

pub(crate) fn tag_name(tag: &str) -> &'static str {
    match tag {
        "modem" => ariel_os_boards::tbox_log::modem::TAG,
        "can" => ariel_os_boards::tbox_log::can::TAG,
        "kernel" => ariel_os_boards::tbox_log::kernel::TAG,
        _ => ariel_os_boards::tbox_log::kernel::TAG,
    }
}

pub(crate) fn enabled(tag: &str) -> bool {
    match tag {
        "modem" => ariel_os_boards::tbox_log::modem::ENABLED,
        "can" => ariel_os_boards::tbox_log::can::ENABLED,
        "kernel" => ariel_os_boards::tbox_log::kernel::ENABLED,
        _ => ariel_os_boards::tbox_log::kernel::ENABLED,
    }
}

pub(crate) fn format_message(args: fmt::Arguments<'_>) -> String<512> {
    let mut message: String<512> = String::new();
    let _ = message.write_fmt(args);
    message
}

#[macro_export]
macro_rules! trace {
    ($fmt:literal $(, $arg:expr)* $(,)?) => {{
        let __tag_key = $crate::tagged_log::tag_for_module(module_path!());
        if $crate::tagged_log::enabled(__tag_key) {
            let __tag = $crate::tagged_log::tag_name(__tag_key);
            let __message = $crate::tagged_log::format_message(format_args!($fmt $(, $arg)*));
            ariel_os::log::trace!("[{}] {}", __tag, ariel_os::log::Display2Format(&__message.as_str()));
        }
    }};
}

#[macro_export]
macro_rules! debug {
    ($fmt:literal $(, $arg:expr)* $(,)?) => {{
        let __tag_key = $crate::tagged_log::tag_for_module(module_path!());
        if $crate::tagged_log::enabled(__tag_key) {
            let __tag = $crate::tagged_log::tag_name(__tag_key);
            let __message = $crate::tagged_log::format_message(format_args!($fmt $(, $arg)*));
            ariel_os::log::debug!("[{}] {}", __tag, ariel_os::log::Display2Format(&__message.as_str()));
        }
    }};
}

#[macro_export]
macro_rules! info {
    ($fmt:literal $(, $arg:expr)* $(,)?) => {{
        let __tag_key = $crate::tagged_log::tag_for_module(module_path!());
        if $crate::tagged_log::enabled(__tag_key) {
            let __tag = $crate::tagged_log::tag_name(__tag_key);
            let __message = $crate::tagged_log::format_message(format_args!($fmt $(, $arg)*));
            ariel_os::log::info!("[{}] {}", __tag, ariel_os::log::Display2Format(&__message.as_str()));
        }
    }};
}

#[macro_export]
macro_rules! warn {
    ($fmt:literal $(, $arg:expr)* $(,)?) => {{
        let __tag_key = $crate::tagged_log::tag_for_module(module_path!());
        if $crate::tagged_log::enabled(__tag_key) {
            let __tag = $crate::tagged_log::tag_name(__tag_key);
            let __message = $crate::tagged_log::format_message(format_args!($fmt $(, $arg)*));
            ariel_os::log::warn!("[{}] {}", __tag, ariel_os::log::Display2Format(&__message.as_str()));
        }
    }};
}

#[macro_export]
macro_rules! error {
    ($fmt:literal $(, $arg:expr)* $(,)?) => {{
        let __tag_key = $crate::tagged_log::tag_for_module(module_path!());
        if $crate::tagged_log::enabled(__tag_key) {
            let __tag = $crate::tagged_log::tag_name(__tag_key);
            let __message = $crate::tagged_log::format_message(format_args!($fmt $(, $arg)*));
            ariel_os::log::error!("[{}] {}", __tag, ariel_os::log::Display2Format(&__message.as_str()));
        }
    }};
}
