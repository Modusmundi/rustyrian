# Upstream findings

Bugs in upstream OpenTyrian (`26e40a2`) found by reading, sanitizers
(`OT_REF_SANITIZE=1` nightly runs) and fuzzing. Security-relevant findings go
through `porting/advisories/` and are not disclosed here until coordinated.

|ID|Location|Finding|Found by|Status|
|---|---|---|---|---|
|UF-1|`network.c`|Security finding in the netplay handshake. Details withheld under coordinated disclosure (`OT-2026-001`).|read; sanitizer reproduction (kept private)|undisclosed|
|UF-2|`tyrian2.c:2942-2943`|`levelEnemyMax` read from the level file without a bound before filling `levelEnemy[40]`.|read|guarded in port (R10)|
|UF-3|`sprite.c:499-532,543-766,820-885`|Sprite offsets from shape tables used without bounds checks; in `JE_loadMainShapeTables` a truncated file or damaged sheet positions give a negative sheet size and crash.|read; differential tests on malformed sheets|guarded in port (R10, D-UB-SPRITE-1)|
|UF-4|`lds_play.c:305-308,656` (`lds_update`, `lds_playsound`)|LDS song data indexes `positions`/`patterns`/`soundbank` unchecked. The freeware `music.mus` song 8 plays instrument 23 with `numpatch = 23`, reading one `SoundBank` past the allocation.|read; differential tests on every song|guarded in port (R10, D-UB-LDS-1)|
|UF-5|`episodes.c:87` (`JE_loadItemDat`)|Item/episode data counts trusted from data files; an episode-4 level file declaring zero levels indexes before `lvlPos` (`lvlPos[lvlNum-1]`).|read; crafted-file tests|guarded in port (R10, D-UB-EPISODES-1)|
|UF-6|`config_file.c` (`sscanf` `%i`/`%x` on a bare `0x`)|Parsing depends on the host libc: Apple/FreeBSD libc pushes the `x` back (value 0, rest unparsed), glibc consumes it.|differential test on macOS and Linux|mirrored per host libc in port (`config_file::scan_integer`)|
|UF-7|`joystick.c:76` (`joystick_axis_reduce`)|`sensitivity` from `opentyrian.cfg` is unbounded; 15 makes the divisor `3000 - 200 * 15` zero (SIGFPE on x86).|read|guarded in port (R10, D-UB-JOYSTICK-SENSITIVITY)|
|UF-8|`video_scale.c:88,137,181,244,307,384`, `video_scale_hqNx.c:193,2997,6861`|`SDL_LockTexture` failure is ignored; the scaler writes through an unset pixel pointer.|read|guarded in port (R10, D-UB-SCALER-LOCK)|
|UF-9|`video.c:187-190` (`window_center_in_display`)|`bounds` is read uninitialised when `SDL_GetDisplayBounds` fails.|read|guarded in port (R10, D-UB-VIDEO-BOUNDS)|
|UF-10|`joystick.c:216-230` (`push_key`)|Only `keysym` is zeroed; `repeat` and the other key-event fields are pushed uninitialised.|read|guarded in port (R10, D-UB-JOYSTICK-PUSHKEY)|
|UF-11|`vga256d.c:28-143` (`JE_pix`, `JE_pix3`, `JE_rectangle`, `JE_barShade`, `JE_barBright`)|Clipping only at the right and bottom edges: negative coordinates write before the pixel buffer (`JE_pix3(0, 0)` writes one byte before it), and `JE_rectangle` with `a > c + 1` calls `memset` with a negative length.|read; differential tests|guarded in port (R10, D-UB-VGA-1)|
|UF-12|`nortvars.c:23-30` (`JE_dBar3`)|The loop counter is a `JE_byte`, so `num >= 255` loops forever.|read|kept (faithful); callers pass player shield/armor values (`varz.c:1078-1104`)|
|UF-13|`lds_play.c:513-517,547,567` (`lds_update`)|An instrument arpeggio longer than 12 overruns `Channel.arp_tab[12]`, and a negative tune gives a negative `frequency` index (not reached by the freeware songs).|read; mutated-song tests|guarded in port (R10, D-UB-LDS-1)|
|UF-14|`nortsong.c:70-85` (`setFrameCount`, `setFrameCount2`)|A timer divisor of 0 or 1 makes `framePeriod` 0 and the `% framePeriod` divides by zero (game code passes speeds ≥ 0x2000).|read|guarded in port (R10, D-UB-TIMER-1)|
|UF-15|`nortsong.c:218-220` (`JE_playSampleNum`)|`samplenum` 0 reads `soundSamples[-1]`; numbers above `SOUND_COUNT` read past the array.|read|guarded in port (R10, D-UB-SND-1)|
|UF-16|`shots.c:338-404` (`player_shot_create`)|`shotMultiPos` left at 9 by a weapon with a larger `max` (the reset only fires above 8) reads index 8 of `JE_WeaponType`'s 8-entry `attack`/`bx`/`by`/… arrays, i.e. the next struct field. Reachable with the freeware data.|read; differential tests|reproduced in port (D-UB-SHOTS-1)|
|UF-17|`helptext.c:108-158` (`JE_helpBox`)|For a one-character message the scan position starts past the length, runs to 255 and wraps, reading the rest of the caller's buffer and the byte before the string; a string of 256 or more bytes never terminates (byte-sized position). No upstream help text triggers either.|read; differential tests|reproduced within the caller's buffer, guarded beyond it (R10, D-UB-HELP-1)|
|UF-18|`pcxload.c:52-54` (`JE_loadPCX`)|`malloc` is not checked for NULL: with a huge file length (a directory named `tshp2.pcx` on ext4 reports LLONG_MAX), `malloc` returns NULL and the decoder then reads through the NULL buffer (SIGSEGV).|Linux CI (oracle)|guarded in port (R10, D-UB-PCX-1)|
|UF-19|`network.c` (`network_connect`, `network_check`)|If the opponent never acknowledges our CONNECT, `network_connect` waits forever and never reads ESC: `network_check` keeps resending the unacknowledged CONNECT and resets `last_out_tick`, so the handshake's NET_RETRY restart never fires.|read; differential tests|kept (faithful)|
|UF-20|`animlib.c:206` (`playAnim`)|The page search trusts the file's page count; more than 256 pages reads past the descriptor array.|read|guarded in port (R10, D-UB-ANIMLIB-PAGES)|
