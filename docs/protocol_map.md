# P map §N=PN. whyyoulying — expand abbrev/cmds when needed.

## Abbreviations (a)
nff=never fire-and-forget bleed=bleeding edge 3s=triple sims chg upd !expose !meta memDB blk=block sct=secret cfl=conflict ttb=throw-the-book-at-it !fileIO=no file I/O 1ver=one version RF=REBUILD FALSE

## Commands (c) — whyyoulying
R=/home/mcochran/whyyoulying
@go=cd $R && cargo run
@t=cargo run -p whyyoulying -- --test
@test=cargo test -p whyyoulying
@b=cargo build --release -p whyyoulying
@check=cargo check -p whyyoulying

## P11 triple-sims
11:Sim1→2→3→4. Implement=default. Docs: TRIPLE_SIMS_*.md. After: upd doc Implementation Summary; @t @b @go §1.

## P3 kova
3:exec>debate;WWKD;Rust;s>f

## P4 whyyoulying-secret
4:sct;!expose

## P9 project-separation
9:whyyoulying=@b. Only rebuild touched project. Ports: N/A (library/CLI).

## P12 ai-slop
12:!utilize→use !leverage→use/apply !facilitate→let/enable !enhance→improve !optimize→improve/tune !comprehensive→full !holistic→drop !robust→solid !seamlessly→drop !empower→enable !streamline→simplify !synergy→drop !paradigm→model !in order to→to. Voice: short active concrete.

## P14 test-binary
14:f49 unit f50 integration f51. --test flag. Same binary. f49:no I/O f50:TempDir DB f51:random port real requests. Colored PASS/FAIL.

## P15 anti-patterns
15:!self-licking !circular !abstraction. Real tests. !traits unless 2+ impl. !wrapper !builder <5 fields. !assume execute verify.

## P16 ci-pipeline
16:Test binary=CI. cargo run -- --test. Stages: compile f49 f50 f51 exit. 0=green 1=red.

## P17 production-binary
17:Same binary main+--test. Core. AES Argon2 if crypto. ctrl_c graceful. LTO strip panic=abort.
