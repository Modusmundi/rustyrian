# Intentional divergences from upstream

The only place behaviour differences between rustyrian and upstream
OpenTyrian (`26e40a2`) are recorded. Kinds: `ub-guard` (rule R10), `security`
(R12), `dead-config` (R11), `platform`, `product` (R14). Each entry names the C
location and the test that proves the divergence.

|ID|Kind|C location|Divergence|Test|
|---|---|---|---|---|
|D-PRODUCT-1|product|`file.c:≈169-237` (`determineUserDirPath`)|User directory is `$XDG_CONFIG_HOME/rustyrian`, then `$HOME/.config/rustyrian` (Unix), `%APPDATA%\rustyrian` (Windows); upstream uses `opentyrian`/`OpenTyrian`. Portable mode unchanged.|`diff:file::determine_user_dir_path` (added with the `file.c` port)|
|D-SEC-1|security|`network.c` (netplay handshake)|Input validation of handshake packets. Details withheld under coordinated disclosure (`OT-2026-001`).|added with the `network.c` port (M3)|
|D-SEC-2|security|`network.c` (`network_check`)|Every received packet is length-checked for its type and dropped when too short.|added with the `network.c` port (M3)|
|D-CONFIG-1|dead-config|`opl.c` (`OPLTYPE_IS_OPL3` paths)|Only the OPL2 path is ported; upstream never defines `OPLTYPE_IS_OPL3`.|added with the `opl.c` port (M2)|
