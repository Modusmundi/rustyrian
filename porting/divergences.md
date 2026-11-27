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
|D-UB-SCALER-LOCK|ub-guard|`video_scale.c:88,137,181,244,307,384`; `video_scale_hqNx.c:193,2997,6861`|When `SDL_LockTexture` fails, C writes through the unset pixel pointer; Rust skips the pixel writes and still calls `SDL_UnlockTexture` (UF-8).|`diff:video_scale::scalers_skip_writes_when_lock_fails`, `diff:video_scale_hqnx::scalers_skip_writes_when_lock_fails`|
|D-UB-VIDEO-BOUNDS|ub-guard|`video.c:187-190` (`window_center_in_display`)|When `SDL_GetDisplayBounds` fails, C reads an uninitialised rect; Rust uses a zero rect, the oracle's observed value (UF-9).|`diff:video::window_center_in_display_matches_c`|
|D-UB-KEYBOARD-SCANCODE|ub-guard|`keyboard.c:214,235`|`keysactive[scancode]` is unchecked in C; a scancode `>= SDL_NUM_SCANCODES` (never produced by SDL) is ignored in Rust.|`diff:keyboard::out_of_range_scancodes_are_ignored`|
|D-UB-JOYSTICK-SENSITIVITY|ub-guard|`joystick.c:76` (`joystick_axis_reduce`)|Divisor `3000 - 200 * sensitivity` is zero for a configured sensitivity of 15; Rust returns 0 instead of dividing by zero (UF-7).|`diff:joystick::axis_reduce_with_zero_divisor_is_zero`|
|D-UB-JOYSTICK-PUSHKEY|ub-guard|`joystick.c:216-230` (`push_key`)|C pushes `repeat` and the other non-`keysym` fields uninitialised; Rust pushes zero (`repeat = false`), the oracle's observed value (UF-10).|`diff:joystick::poll_and_push_joysticks_match_c`|
