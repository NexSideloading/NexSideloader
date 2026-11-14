<div align="center">

# NexSideloader

**Install SideStore and LiveContainer directly on your iPhone. No PC required.**

Original SideInstaller Bootstrap by FrizzleM, modified by NovaDev404.

</div>

---

## Overview

NexSideloader is a modified version of siboot (SideInstaller Bootstrap) that enables you to install SideStore and LiveContainer on your iPhone without needing a computer. This repository contains three main components:

- **[SideInstaller](./SideInstaller/)** - A SwiftUI app that handles the entire setup process on-device (upstream)
- **[minis](./minis/)** - NexSideloader for Open Minis, installs SideStore from Open Minis terminal
- **[rootshell](./rootshell/)** - siboot-pair for rootshell, pairs the iPhone with itself

### Quick Start

To use the original SideInstaller, follow the instructions at: https://sideinstaller.net/install.html

**Requirements:** iOS 27+ is required

---

## Components

### SideInstaller

The main SwiftUI application that provides a user-friendly interface for installing SideStore and LiveContainer. This is the upstream project by FrizzleM.

**[Read the SideInstaller documentation →](./SideInstaller/README.md)**

**Features:**
- Install SideStore and LiveContainer directly on your iPhone
- No PC required
- Handles pairing, provisioning, and installation automatically
- Open source and fully auditable
- Your Apple credentials never leave your device

### minis (NexSideloader for Open Minis)

Installs SideStore or SideStore+LiveContainer onto the iPhone it runs on, signed with your own Apple Account, from the Linux shell of Open Minis (free, App Store). No computer or certificate required. Modified from siboot to install SideStore instead of SideInstaller.

**[Read the minis documentation →](./minis/README.md)**

**Features:**
- Runs from Open Minis terminal
- Uses siboot-pair from rootshell for pairing
- Interactive web page interface
- Terminal-based option available
- Includes Developer Mode toggle
- Splits pairing file into SideStore's three formats (Lockdown, RemoteRP, ALTPairingFile)

### rootshell (siboot-pair)

NexSideloader's pairing step as a WASM program for rootshell. It pairs the iPhone with itself, then hands the pairing file to NexSideloader in Open Minis.

**[Read the rootshell documentation →](./rootshell/README.md)**

**Features:**
- Pairs iPhone with itself via mDNS
- WASM program for rootshell
- Provides pairing file to NexSideloader
- Large code display and notifications
- Copy log to clipboard support

---

## How It Works Together

The three components work together to provide a complete PC-free installation experience:

1. **rootshell** pairs your iPhone with itself using siboot-pair
2. **minis** uses the pairing file to sign and install SideStore/LiveContainer
3. **SideInstaller** provides the upstream all-in-one solution with a polished UI

---

## Requirements

- iPhone or iPad on iOS 27+ (iOS 17.4+ with a pairing file made on a computer)
- Wi-Fi (first time only)
- Your Apple account credentials (not collected)
- Developer Mode enabled

---

## Modifications

This fork of siboot makes the following changes to install SideStore instead of SideInstaller:

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
> - Website: https://sideinstaller.net/
> - Repository: https://github.com/FrizzleM/SideInstaller
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
