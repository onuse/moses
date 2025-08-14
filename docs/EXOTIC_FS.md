# Exotic & Historical Filesystems Roadmap

## Vision
Moses aims to become the universal filesystem formatter, supporting not just modern filesystems but also historical, exotic, and specialized formats. This would make Moses invaluable for:
- Retro computing enthusiasts
- Data recovery specialists
- Digital archivists
- Game console modders
- Embedded system developers
- Computer historians

## Implementation Difficulty Scale
- 🟢 **Easy**: Tools exist, well-documented
- 🟡 **Medium**: Some tools available, moderate documentation
- 🔴 **Hard**: Few tools, poor documentation
- ⚫ **Expert**: Reverse engineering required, almost no documentation

---

## 📼 Game Console Filesystems

### Sony PlayStation
- **PS1 Memory Card FS** 🟡
  - 128KB cards with proprietary block structure
  - Used for PS1, PS2 (backwards compatible)
  - Tools: MemcardRex, PSXMemTool

- **PS2 Memory Card FS (MCFS)** 🟡
  - ECC protected filesystem
  - Hierarchical structure
  - Tools: mymc, ps2mc-gui

- **PSP Memory Stick (MSPFS)** 🟢
  - Modified FAT32 with Sony extensions
  - Special directories for games/saves
  - Tools: Standard FAT32 + special structure

- **PS Vita Memory Card** 🔴
  - Proprietary exFAT variant
  - Encrypted filesystem
  - Tools: vita-mcfs (limited)

### Nintendo
- **GameCube Memory Card** 🟡
  - Fixed block allocation
  - No real filesystem, just block storage
  - Tools: GCMM, Dolphin emulator

- **Wii/Wii U WBFS** 🟢
  - Wii Backup File System
  - Optimized for large game storage
  - Tools: wbfs_file, wit

- **3DS SD Card Format** 🔴
  - Encrypted FAT32 with Nintendo's layer
  - CTR partition scheme
  - Tools: GodMode9, custom_install

- **Switch SD Card (Nintendo emuMMC)** 🟡
  - FAT32/exFAT with special partitioning
  - Hidden partition for emuNAND
  - Tools: Hekate, NxNandManager

### Microsoft Xbox
- **Xbox FATX** 🟢
  - Modified FAT16/32 for original Xbox
  - 42-character filename limit
  - Tools: FATXplorer, xboxhdm

- **Xbox 360 XTAF** 🟡
  - Xbox Tape Archive Format
  - Used on memory units and HDDs
  - Tools: Horizon, Modio

- **Xbox 360 STFS** 🔴
  - Secure Transacted File System
  - CON/LIVE/PIRS packages
  - Tools: Velocity, Le Fluffie

### Other Consoles
- **Dreamcast VMU FS** 🟡
  - Visual Memory Unit filesystem
  - 128KB with icon support
  - Tools: VMU Tool PC, Dream Explorer

- **Sega Saturn Backup RAM** 🔴
  - Proprietary format for saves
  - Very limited documentation
  - Tools: Sega Saturn Patcher

---

## 💾 Retro Computer Filesystems

### Commodore
- **Commodore 1541/1571/1581** 🟢
  - CBM DOS filesystems
  - D64/D71/D81 disk images
  - Tools: cc1541, c1541 (VICE)

- **Amiga OFS/FFS** 🟢
  - Original/Fast File System
  - Still used by enthusiasts
  - Tools: ADFOpus, xdftool

- **Amiga PFS/PFS2/PFS3** 🟡
  - Professional File System
  - Better performance than FFS
  - Tools: PFSformat

- **Amiga SFS** 🟡
  - Smart File System
  - Modern Amiga filesystem
  - Tools: SFSformat

### Apple
- **Apple II DOS 3.3** 🟢
  - 140KB floppy format
  - VTOC (Volume Table of Contents)
  - Tools: CiderPress, AppleCommander

- **Apple ProDOS** 🟢
  - Hierarchical filesystem
  - 32MB volume limit
  - Tools: CiderPress, ProDOS utilities

- **Apple HFS** 🟢
  - Hierarchical File System (Classic Mac)
  - Pre-Mac OS X standard
  - Tools: hfsutils, hfsplus

- **Apple MFS** 🔴
  - Macintosh File System (original 1984)
  - Flat file structure
  - Tools: Mini vMac, Basilisk II

- **Apple Lisa FS** ⚫
  - Lisa Office System filesystem
  - Extremely rare
  - Tools: LisaEm (limited)

### Atari
- **Atari DOS 2.x** 🟢
  - 8-bit Atari computers
  - 90KB/130KB floppies
  - Tools: dir2atr, atr2unix

- **Atari TOS/GEMDOS** 🟢
  - ST/TT/Falcon filesystem
  - FAT12/16 compatible variant
  - Tools: Standard FAT tools

- **SpartaDOS FS** 🟡
  - Advanced Atari 8-bit DOS
  - Subdirectories, timestamps
  - Tools: SpartaDOS X

### Sinclair
- **ZX Spectrum +3DOS** 🟡
  - CP/M compatible
  - 173KB/710KB formats
  - Tools: CPCDiskXP, SAMdisk

- **ZX Spectrum TAP/TZX** 🟢
  - Tape filesystem formats
  - Sequential access
  - Tools: taptools, tzxtools

- **QL QDOS** 🔴
  - Sinclair QL filesystem
  - Microdrive cartridges
  - Tools: QLTools

### Other 8-bit
- **MSX-DOS 1/2** 🟢
  - CP/M and MS-DOS compatible
  - FAT12 based
  - Tools: MSXDiskExplorer

- **TRS-80 TRSDOS** 🟡
  - Various versions (Model I/III/4)
  - Different disk formats
  - Tools: trstools, trsread

- **BBC Micro DFS/ADFS** 🟡
  - Disk/Advanced Disk Filing System
  - Acorn computers
  - Tools: bbcim, dfs2img

---

## 🖥️ Workstation & Server Filesystems

### Unix/Mainframe
- **UFS/UFS2** 🟢
  - Unix File System (BSD)
  - Still actively used
  - Tools: newfs, mkfs.ufs

- **Minix FS** 🟢
  - Original Linux filesystem base
  - Educational value
  - Tools: mkfs.minix

- **Xenix FS** 🔴
  - Microsoft's Unix filesystem
  - Historical interest
  - Tools: Limited

- **WORM FS** 🔴
  - Write Once Read Many
  - Optical storage
  - Tools: Specialized

- **QNX4/QNX6 FS** 🟡
  - Real-time OS filesystem
  - Embedded systems
  - Tools: mkqnx6fs

### IBM
- **JFS** 🟢
  - Journaled File System (AIX/OS/2)
  - Still maintained
  - Tools: mkfs.jfs

- **HPFS** 🟡
  - High Performance File System (OS/2)
  - Historical Windows NT support
  - Tools: Limited on modern systems

- **VM/CMS** ⚫
  - IBM mainframe filesystem
  - Very specialized
  - Tools: Mainframe only

### Digital/DEC
- **Files-11** 🔴
  - VMS/OpenVMS filesystem
  - Complex structure
  - Tools: ODS2 reader

- **RT-11** 🔴
  - DEC PDP-11 filesystem
  - Historical interest
  - Tools: rt11fs, putr

### Sun/Oracle
- **QFS** 🔴
  - Quick File System
  - High-performance
  - Tools: sammkfs

### SGI
- **EFS** 🔴
  - Extent File System (early IRIX)
  - Predecessor to XFS
  - Tools: mkfs_efs (rare)

---

## 💿 Optical Media Filesystems

### CD/DVD Formats
- **ISO 9660** 🟢
  - Standard CD-ROM filesystem
  - Rock Ridge/Joliet extensions
  - Tools: mkisofs, genisoimage

- **UDF** 🟢
  - Universal Disk Format
  - DVD/Blu-ray standard
  - Tools: mkudffs

- **HFS+ Hybrid** 🟡
  - Mac/PC hybrid CDs
  - Dual filesystem
  - Tools: hfsutils + mkisofs

- **El Torito** 🟢
  - Bootable CD specification
  - BIOS/UEFI boot
  - Tools: mkisofs with -b

### Proprietary Optical
- **3DO Opera FS** 🔴
  - 3DO game console format
  - CD-ROM based
  - Tools: 3DOTools

- **CD-i** 🔴
  - Philips CD-i format
  - Green Book standard
  - Tools: Very limited

---

## 🎮 Arcade & Embedded

### Arcade Systems
- **NAOMI GD-ROM** 🔴
  - Sega NAOMI/Dreamcast
  - High-density CD format
  - Tools: gdi2data

- **Taito Type X** 🟡
  - PC-based arcade
  - Custom partition scheme
  - Tools: TTX tools

### Embedded Systems
- **YAFFS/YAFFS2** 🟡
  - Yet Another Flash File System
  - NAND flash optimized
  - Tools: mkyaffs2image

- **UBIFS** 🟢
  - Unsorted Block Image File System
  - Modern flash filesystem
  - Tools: mkfs.ubifs

- **CramFS** 🟢
  - Compressed ROM filesystem
  - Read-only embedded
  - Tools: mkcramfs

- **SquashFS** 🟢
  - Compressed read-only
  - Router firmware
  - Tools: mksquashfs

- **RomFS** 🟢
  - Simple ROM filesystem
  - Embedded Linux
  - Tools: genromfs

---

## 🔬 Specialized/Research Filesystems

### Distributed/Network
- **AFS** 🔴
  - Andrew File System
  - Distributed filesystem
  - Tools: OpenAFS

- **Coda** 🔴
  - Disconnected operation
  - Mobile computing
  - Tools: Coda client/server

### Encrypted
- **StegFS** ⚫
  - Steganographic filesystem
  - Plausible deniability
  - Tools: Research only

- **TCFS** 🔴
  - Transparent Cryptographic FS
  - NFS encryption layer
  - Tools: Historical

### Database-like
- **BeFS** 🟡
  - BeOS/Haiku filesystem
  - Database-like attributes
  - Tools: mkbfs (Haiku)

- **WinFS** ⚫
  - Windows Future Storage
  - Never released
  - Tools: None (vaporware)

### Academic/Experimental
- **WAFL** 🔴
  - Write Anywhere File Layout (NetApp)
  - Snapshot technology
  - Tools: NetApp only

- **Fossil** 🔴
  - Plan 9 archival filesystem
  - Venti backing store
  - Tools: Plan 9 only

- **LFS** 🔴
  - Log-structured File System
  - BSD experimental
  - Tools: newfs_lfs

---

## 📱 Mobile & PDA Filesystems

### Palm OS
- **Palm Database Format** 🟡
  - PDB/PRC files
  - Not a true filesystem
  - Tools: pilot-tools

### Windows Mobile
- **TFFS** 🔴
  - Transaction-Safe FAT
  - Flash-optimized FAT
  - Tools: CE tools

### Symbian
- **LFFS** 🔴
  - Symbian filesystem
  - NAND flash optimized
  - Tools: Very limited

---

## 🎯 Implementation Priority

### Phase 1: Popular Retro (High demand, good documentation)
1. Amiga OFS/FFS
2. Apple HFS
3. Commodore 1541
4. ISO 9660 + extensions
5. Xbox FATX

### Phase 2: Console Essentials (Gaming community)
1. PS1/PS2 Memory Cards
2. GameCube Memory Cards
3. Wii WBFS
4. Dreamcast VMU

### Phase 3: Unix/Linux Variants
1. UFS/UFS2
2. Minix FS
3. JFS
4. BeFS

### Phase 4: Embedded/Modern
1. YAFFS2
2. UBIFS
3. SquashFS
4. CramFS

### Phase 5: Ultra-Exotic (For completeness)
1. Lisa FS
2. Files-11
3. 3DO Opera FS
4. StegFS

---

## 🛠️ Implementation Strategy

### Plugin Architecture
```rust
trait ExoticFormatter: FilesystemFormatter {
    fn era(&self) -> Era;           // 1970s, 1980s, 1990s, etc.
    fn platform(&self) -> Platform; // Commodore, Apple, Atari, etc.
    fn rarity(&self) -> Rarity;     // Common, Rare, UltraRare
    fn emulator_compatible(&self) -> Vec<String>; // VICE, UAE, etc.
}
```

### Community Contributions
- Create plugin SDK for exotic formats
- Bounty program for rare formats
- Partnership with retro communities
- Integration with emulators

### Testing Strategy
- Emulator verification
- Real hardware testing (community)
- Format conversion validation
- Round-trip testing

---

## 📚 Resources

### Communities
- [English Amiga Board](http://eab.abime.net/)
- [AtariAge Forums](https://atariage.com/forums/)
- [Vintage Computer Federation](https://www.vcfed.org/)
- [r/retrobattlestations](https://reddit.com/r/retrobattlestations)

### Documentation
- [Filesystem Hierarchy](http://www.filesystems.org/)
- [Linux Filesystem Development](https://www.kernel.org/doc/Documentation/filesystems/)
- [Computer History Museum](https://computerhistory.org/)

### Tools/Libraries
- [The Unarchiver](https://theunarchiver.com/) - Multi-format support
- [HxC Floppy Emulator](https://hxc2001.com/) - Universal floppy tool
- [Kryoflux](https://kryoflux.com/) - Flux-level disk imaging

---

## 🎖️ Achievement Goals

### "Filesystem Archaeologist"
Support 50+ historical formats

### "Console Master"
Support all major game console formats

### "Time Traveler"
Support formats from 5+ decades

### "Universal Formatter"
Support 100+ total formats

### "Digital Preservationist"
Enable data recovery from 25+ obsolete formats

---

## Vision Statement

> "Moses will become the Rosetta Stone of filesystems - a single tool that can read, write, and translate between any storage format ever created. From the latest NVMe drives to 1970s floppy disks, from game console memory cards to mainframe tapes, Moses will preserve our digital heritage and make it accessible to all."

This is not just about nostalgia - it's about:
- **Digital Preservation**: Keeping old data accessible
- **Education**: Learning from filesystem evolution
- **Research**: Understanding storage history
- **Recovery**: Salvaging data from obsolete media
- **Compatibility**: Bridging old and new systems

The goal: **If it stored data, Moses can format it.**