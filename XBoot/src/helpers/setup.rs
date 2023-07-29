// Copyright (c) ChefKiss Inc 2021-2023. Licensed under the Thou Shalt Not Profit License version 1.5. See LICENSE for details.

use amd64::paging::pml4::PML4;
use uefi::{
    proto::console::text::Key,
    table::boot::{EventType, TimerTrigger, Tpl},
    Char16,
};

pub fn setup() {
    trace!("Setting up higher-half paging mappings:");
    trace!("    1. Turning off write protection...");
    unsafe {
        core::arch::asm!(
            "mov rax, cr0",
            "and rax, {wp_bit}",
            "mov cr0, rax",
            wp_bit = const !(1u64 << 16),
            options(nostack, preserves_flags, nomem),
        );
    }

    trace!("    2. Modifying paging mappings to map higher-half...");
    unsafe { super::PML4::get().map_higher_half() }
}

pub fn check_boot_flags() -> (bool, bool) {
    let st = unsafe { uefi_services::system_table().as_mut() };
    let timer = match unsafe {
        st.boot_services()
            .create_event(EventType::TIMER, Tpl::CALLBACK, None, None)
    } {
        Ok(v) => v,
        Err(e) => {
            warn!("Failed to create timer: {e}.");
            return (false, false);
        }
    };
    if let Err(e) = st
        .boot_services()
        .set_timer(&timer, TimerTrigger::Relative(5 * 1000 * 1000))
    {
        warn!("Failed to set timer: {e}.");
        st.boot_services().close_event(timer).unwrap();
        return (false, false);
    };
    let mut events = unsafe {
        [
            timer.unsafe_clone(),
            st.stdin().wait_for_key_event().unsafe_clone(),
        ]
    };
    let i = match st.boot_services().wait_for_event(&mut events) {
        Ok(v) => v,
        Err(e) => {
            warn!("Failed to wait for event: {e}.");
            st.boot_services().close_event(timer).unwrap();
            return (false, false);
        }
    };

    st.boot_services().close_event(timer).unwrap();
    if i == 0 {
        return (false, false);
    }

    let mut verbose = false;
    let mut serial_enabled = false;
    while let Ok(v) = st.stdin().read_key() {
        match v {
            Some(Key::Printable(v)) if v == Char16::try_from('v').unwrap() => {
                verbose = true;
                break;
            }
            Some(Key::Printable(v)) if v == Char16::try_from('s').unwrap() => {
                serial_enabled = true;
                break;
            }
            _ => {}
        }
    }
    (verbose, serial_enabled)
}

pub fn get_rsdp() -> *const u8 {
    let mut iter = unsafe { uefi_services::system_table().as_mut().config_table().iter() };
    let rsdp: *const u8 = iter
        .find(|ent| ent.guid == uefi::table::cfg::ACPI2_GUID)
        .unwrap_or_else(|| {
            iter.find(|ent| ent.guid == uefi::table::cfg::ACPI_GUID)
                .unwrap()
        })
        .address
        .cast();
    super::pa_to_kern_va(rsdp)
}
