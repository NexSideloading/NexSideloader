<div align="center">

# NexSideloader

**Install SideStore and LiveContainer directly on your iPhone. No PC. No certs.**

Original SideInstaller Bootstrap by FrizzleM, modified by NovaDev404.

**Requirements:** iOS 27+ is required

</div>

---

## Quick Start
1. Install [Open Minis](https://apps.apple.com/app/id6759188481) and [LocalDevVPN](https://apps.apple.com/app/id6755608044) from the App Store.
2. Go to Settings → Privacy & Security, scroll to the bottom and tap on Developer Mode, then enable it.
3. Once Developer Mode is enabled, open LocalDevVPN and turn on the VPN.
4. Open Open Minis, tap the console icon at the top right, then press Shell Terminal.
5. Copy and paste this command into the terminal: `wget -qO- https://sideloading.net/sideloader.sh | sh`.
7. Press Enter and follow the steps to install SideStore or SideStore + Livecontainer  .

> Note: This method is for iOS 27 only!

---

## Overview

NexSideloader is a modified version of siboot (SideInstaller Bootstrap) that enables you to install SideStore and LiveContainer on your iPhone without needing a computer. This repository contains three main components:

- **[SideInstaller](./SideInstaller/)** - A SwiftUI app that handles the entire setup process on-device (upstream)
- **[minis](./minis/)** - NexSideloader for Open Minis, installs SideStore from Open Minis terminal

---

## Minis (NexSideloader for Open Minis)

Installs SideStore or SideStore+LiveContainer onto the iPhone it runs on, signed with your own Apple Account, from the Linux shell of Open Minis (free, App Store). No computer or certificate required. Modified from siboot to install SideStore instead of SideInstaller.

**[Read the minis documentation →](./minis/README.md)**

**Features:**
- Runs from Open Minis terminal
- Uses siboot-pair from rootshell for pairing
- Interactive web page interface
- Terminal-based option available
- Includes Developer Mode toggle
- Splits pairing file into SideStore's three formats (Lockdown, RemoteRP, ALTPairingFile)

---

## How It Works

 **minis** pairs your device, and uses the pairing file to sign and install SideStore/LiveContainer without any other steps

---

## Requirements

- iPhone or iPad on iOS 27+
- Wi-Fi (first time only)
- Your Apple account credentials (not collected)
- Developer Mode enabled

---

## Modifications

This fork of siboot makes the following changes to install SideStore instead of SideInstaller:

- SideInstaller not needed. Install SideStore direct from **Open Minis**
- Added app selection (SideStore or SideStore+LiveContainer)
- Updated download URLs to SideStore's nightly releases
- Splits pairing file into SideStore's three formats:
  - `PairingFile_Lockdown.plist` - Lockdown protocol record
  - `PairingFile_RemoteRP.plist` - RPPairing protocol record
  - `ALTPairingFile.mobiledevicepairing` - Legacy format
- Writes pairing files to app's Documents folder instead of Application Support
- Renamed to NexSideloader with attribution to original author

---

## Is It Safe?

All components are built with safety and privacy in mind:

- **Your Apple credentials never leave your device** - used locally only
- **No server, no analytics, no account** - nothing is collected
- **Fully auditable** - every line can be verified in the source code

> [!CAUTION]
> Only install from official sources. This project is open source, which means anyone can fork the code and insert malicious code. The only official builds are from:
> - Website: https://sideloading.net/sidestoreinstaller/
> - Repository: https://github.com/NexSideloading/NexSideloader
>
> This is a modified fork by NovaDev404. Review the changes before using.

---

## Contributing

Issues and pull requests are welcome. Please see individual component READMEs for specific contribution guidelines.

---

## License

See [SideInstaller/LICENSE.md](./SideInstaller/LICENSE.md). In short: you may read, modify, and share the source, with attribution, for non-commercial purposes. Redistributing the official builds (IPAs) or monetizing the project in any form requires written permission.

---

<div align="center">

**Original SideInstaller Bootstrap by [FrizzleM](https://github.com/FrizzleM)**

**Modified by [NovaDev404](https://github.com/NovaDev404)**

</div>
