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
