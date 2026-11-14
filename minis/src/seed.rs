//! Step 7: hand the newly installed SideStore this pairing.
//! Splits the pairing file into three formats and writes them to SideStore's
//! Documents folder: PairingFile_Lockdown.plist, PairingFile_RemoteRP.plist,
//! and ALTPairingFile.mobiledevicepairing.

use std::path::Path;

use idevice::afc::opcode::AfcFopenMode;
use idevice::afc::AfcClient;
use idevice::house_arrest::HouseArrestClient;
use idevice::RsdService;

use crate::fail::{chain, Fail, Result};
use crate::sidestore::split_pairing_file;
use crate::tunnel::Connection;
use crate::ui;

/// SideStore's Documents folder.
const DOCUMENTS_PATH: &str = "/Documents";
/// Lockdown pairing file for SideStore.
const LOCKDOWN_FILE: &str = "PairingFile_Lockdown.plist";
/// RPPairing file for SideStore.
const REMOTE_FILE: &str = "PairingFile_RemoteRP.plist";
/// Legacy pairing file for SideStore.
const LEGACY_FILE: &str = "ALTPairingFile.mobiledevicepairing";

pub async fn hand_over(
    connection: &mut Connection,
    bundle_id: &str,
    pairing_path: &Path,
    udid: &str,
) -> Result<()> {
    ui::activity("opening SideStore's container");
    let pairing = std::fs::read(pairing_path).map_err(|e| {
        Fail::new(
            format!("Couldn't read {}.", pairing_path.display()),
            e.to_string(),
        )
    })?;

    let house_arrest =
        HouseArrestClient::connect_rsd(&mut connection.adapter, &mut connection.handshake)
            .await
            .map_err(|e| Fail::new("Couldn't reach house_arrest on this iPhone.", chain(&e)))?;
    let mut afc = house_arrest.vend_container(bundle_id).await.map_err(|e| {
        Fail::new(
            format!("Couldn't open {bundle_id}'s container. Import the pairing file in SideStore instead: it's at ~/.siboot/rp_pairing_file.plist in Minis (kept from original siboot)."),
            chain(&e),
        )
    })?;

    // Ensure Documents folder exists
    let _ = afc.mk_dir(DOCUMENTS_PATH).await;

    ui::activity("splitting pairing file into SideStore's formats");
    let files = split_pairing_file(&pairing, Some(udid))?;

    let mut written_count = 0;

    // Write Lockdown file if we have it
    if let Some(lockdown_data) = files.lockdown {
        let path = format!("{DOCUMENTS_PATH}/{LOCKDOWN_FILE}");
        let written = write_verified(&mut afc, &path, &lockdown_data).await?;
        ui::ok(&format!(
            "PairingFile_Lockdown.plist written ({written} bytes)"
        ));
        written_count += 1;
    }

    // Write RemoteRP file if we have it
    if let Some(remote_data) = files.remote {
        let path = format!("{DOCUMENTS_PATH}/{REMOTE_FILE}");
        let written = write_verified(&mut afc, &path, &remote_data).await?;
        ui::ok(&format!(
            "PairingFile_RemoteRP.plist written ({written} bytes)"
        ));
        written_count += 1;
    }

    // Always write legacy file
    let legacy_path = format!("{DOCUMENTS_PATH}/{LEGACY_FILE}");
    let legacy_written = write_verified(&mut afc, &legacy_path, &files.legacy).await?;
    ui::ok(&format!(
        "ALTPairingFile.mobiledevicepairing written ({legacy_written} bytes)"
    ));
    written_count += 1;

    ui::done(&format!(
        "{} pairing file(s) written to SideStore's Documents folder",
        written_count
    ));

    Ok(())
}

async fn read_if_present(afc: &mut AfcClient, path: &str) -> Result<Option<Vec<u8>>> {
    if afc.get_file_info(path).await.is_err() {
        return Ok(None);
    }
    let mut file = afc
        .open(path, AfcFopenMode::RdOnly)
        .await
        .map_err(|e| Fail::new(format!("Couldn't open {path}."), chain(&e)))?;
    let bytes = file
        .read_entire()
        .await
        .map_err(|e| Fail::new(format!("Couldn't read {path}."), chain(&e)))?;
    let _ = file.close().await;
    Ok(Some(bytes))
}

/// Write `data` to `path`, then read it back and compare. Returns its length.
async fn write_verified(afc: &mut AfcClient, path: &str, data: &[u8]) -> Result<usize> {
    let mut file = afc
        .open(path, AfcFopenMode::WrOnly)
        .await
        .map_err(|e| Fail::new(format!("Couldn't create {path}."), chain(&e)))?;
    file.write_entire(data)
        .await
        .map_err(|e| Fail::new(format!("Couldn't write {path}."), chain(&e)))?;
    file.close()
        .await
        .map_err(|e| Fail::new(format!("Couldn't write {path}."), chain(&e)))?;
    match read_if_present(afc, path).await? {
        Some(back) if back == data => Ok(data.len()),
        Some(back) => Err(Fail::new(
            format!("{path} reads back different from what was written."),
            format!("wrote {} bytes, read {}", data.len(), back.len()),
        )),
        None => Err(Fail::new(format!("{path} vanished after writing."), "")),
    }
}
