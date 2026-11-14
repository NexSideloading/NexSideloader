//! Split pairing file into SideStore's three formats.

use crate::fail::{Fail, Result};

/// The keys required for a lockdown pairing record.
const LOCKDOWN_KEYS: &[&str] = &[
    "WiFiMACAddress", "SystemBUID", "RootPrivateKey", "HostPrivateKey", "HostID",
    "RootCertificate", "UDID", "EscrowBag", "HostCertificate", "DeviceCertificate",
];

/// The keys required for an RPPairing record.
const REMOTE_KEYS: &[&str] = ["private_key", "public_key", "identifier"];

/// All RPPairing keys, including optional ones.
const REMOTE_ALL_KEYS: &[&str] = ["private_key", "public_key", "identifier", "alt_irk"];

/// The three pairing files SideStore needs.
pub struct SideStorePairingFiles {
    /// Lockdown protocol record (PairingFile_Lockdown.plist)
    pub lockdown: Option<Vec<u8>>,
    /// RPPairing protocol record (PairingFile_RemoteRP.plist)
    pub remote: Option<Vec<u8>>,
    /// Legacy format (ALTPairingFile.mobiledevicepairing)
    pub legacy: Vec<u8>,
}

/// Split a pairing file into SideStore's three formats.
pub fn split_pairing_file(data: &[u8], udid: Option<&str>) -> Result<SideStorePairingFiles> {
    let parsed = plist::from_bytes::<plist::Dictionary>(data)
        .map_err(|e| Fail::new("The pairing file isn't a valid plist.", e.to_string()))?;

    // Extract lockdown keys (everything that's not RPPairing)
    let mut lockdown_dict = plist::Dictionary::new();
    let mut has_lockdown = false;
    for (key, value) in parsed.iter() {
        if !REMOTE_ALL_KEYS.contains(&key.as_str()) {
            lockdown_dict.insert(key.clone(), value.clone());
            has_lockdown = true;
        }
    }

    // Add UDID if missing and provided
    if let Some(udid) = udid {
        if !udid.is_empty() {
            lockdown_dict
                .entry("UDID".into())
                .or_insert_with(|| udid.into());
        }
    }

    // Check if lockdown record is complete
    let lockdown_data = if LOCKDOWN_KEYS.iter().all(|k| lockdown_dict.contains_key(*k)) {
        Some(serialize_plist(&lockdown_dict)?)
    } else {
        None
    };

    // Extract RPPairing keys
    let mut remote_dict = plist::Dictionary::new();
    let mut has_remote = false;
    for key in REMOTE_KEYS {
        if let Some(value) = parsed.get(*key) {
            remote_dict.insert((*key).into(), value.clone());
            has_remote = true;
        }
    }

    // Add optional alt_irk if present
    if let Some(value) = parsed.get("alt_irk") {
        remote_dict.insert("alt_irk".into(), value.clone());
    }

    let remote_data = if has_remote {
        Some(serialize_plist(&remote_dict)?)
    } else {
        None
    };

    // Legacy format is the original file (or just one of the records if only one exists)
    let legacy_data = data.to_vec();

    Ok(SideStorePairingFiles {
        lockdown: lockdown_data,
        remote: remote_data,
        legacy: legacy_data,
    })
}

/// Serialize a plist dictionary to XML bytes.
fn serialize_plist(dict: &plist::Dictionary) -> Result<Vec<u8>> {
    plist::to_writer_xml(Vec::new(), dict)
        .map_err(|e| Fail::new("Couldn't serialize pairing record.", e.to_string()))
}
