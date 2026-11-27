# Upstream findings

Bugs in upstream OpenTyrian (`26e40a2`) found by reading, sanitizers
(`OT_REF_SANITIZE=1` nightly runs) and fuzzing. Security-relevant findings go
through `porting/advisories/` and are not disclosed here until coordinated.

|ID|Location|Finding|Found by|Status|
|---|---|---|---|---|
|UF-1|`network.c`|Security finding in the netplay handshake. Details withheld under coordinated disclosure (`OT-2026-001`).|read; sanitizer reproduction (kept private)|undisclosed|
|UF-2|`tyrian2.c:2942-2943`|`levelEnemyMax` read from the level file without a bound before filling `levelEnemy[40]`.|read|guarded in port (R10)|
|UF-3|`sprite.c:550-731`|Sprite offsets from shape tables used without bounds checks.|read|guarded in port (R10)|
|UF-4|`lds_play.c:305-308,656`|LDS song indices from `music.mus` used without bounds checks.|read|guarded in port (R10)|
|UF-5|`episodes.c:87`|Item/episode data counts trusted from data files.|read|guarded in port (R10)|
|UF-6|`config_file.c` (`sscanf` `%i`/`%x` on a bare `0x`)|Parsing depends on the host libc: Apple/FreeBSD libc pushes the `x` back (value 0, rest unparsed), glibc consumes it.|differential test on macOS and Linux|mirrored per host libc in port (`config_file::scan_integer`)|
|UF-7|`joystick.c:76` (`joystick_axis_reduce`)|`sensitivity` from `opentyrian.cfg` is unbounded; 15 makes the divisor `3000 - 200 * 15` zero (SIGFPE on x86).|read|guarded in port (R10, D-UB-JOYSTICK-SENSITIVITY)|
|UF-8|`video_scale.c:88,137,181,244,307,384`, `video_scale_hqNx.c:193,2997,6861`|`SDL_LockTexture` failure is ignored; the scaler writes through an unset pixel pointer.|read|guarded in port (R10, D-UB-SCALER-LOCK)|
|UF-9|`video.c:187-190` (`window_center_in_display`)|`bounds` is read uninitialised when `SDL_GetDisplayBounds` fails.|read|guarded in port (R10, D-UB-VIDEO-BOUNDS)|
|UF-10|`joystick.c:216-230` (`push_key`)|Only `keysym` is zeroed; `repeat` and the other key-event fields are pushed uninitialised.|read|guarded in port (R10, D-UB-JOYSTICK-PUSHKEY)|
