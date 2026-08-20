//! System Restore point integration (Phase 2): `SRSetRestorePointW` before
//! destructive operations, as an extra safety net on top of Optix snapshots.
//! Best-effort: fails cleanly when System Protection is disabled.

use crate::error::{OptixError, Result};

/// Result of attempting to create a restore point.
#[cfg(windows)]
pub fn create_restore_point(description: &str) -> Result<Option<u32>> {
    use windows_sys::Win32::System::Restore::{
        SRSetRestorePointW, RESTOREPOINTINFOW, STATEMGRSTATUS, BEGIN_SYSTEM_CHANGE,
        MODIFY_SETTINGS,
    };

    let mut description_utf16: Vec<u16> = description.encode_utf16().collect();
    if description_utf16.len() >= 256 {
        description_utf16.truncate(255);
    }
    if description_utf16.is_empty() {
        description_utf16.push(b' '.into());
    }
    description_utf16.push(0);

    let mut info = RESTOREPOINTINFOW {
        dwEventType: BEGIN_SYSTEM_CHANGE,
        dwRestorePtType: MODIFY_SETTINGS,
        llSequenceNumber: 0,
        szDescription: [0u16; 256],
    };
    for (i, ch) in description_utf16.iter().take(255).enumerate() {
        info.szDescription[i] = *ch;
    }

    let mut status = STATEMGRSTATUS {
        nStatus: 0,
        llSequenceNumber: 0,
    };
    let ok = unsafe { SRSetRestorePointW(&info, &mut status) };
    if ok == 0 {
        // STATEMGRSTATUS is 1-byte packed in windows-sys; copy the field out
        // before formatting (referencing a packed field is unaligned).
        let n = status.nStatus;
        return Err(OptixError::Windows(format!(
            "System Restore point failed (error {n}) — is System Protection enabled?"
        )));
    }
    Ok((status.llSequenceNumber != 0).then_some(status.llSequenceNumber as u32))
}

#[cfg(not(windows))]
pub fn create_restore_point(_description: &str) -> Result<Option<u32>> {
    Err(OptixError::UnsupportedPlatform("System Restore point".into()))
}