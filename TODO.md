## Major:

- [x] implement `start_uci` 
- [x] implement `new_game`
- [x] implement `set_position`
- [x] implement `go_infinite`
- [x] read stdout in separate task
- [x] implement `get_evaluation`
- [x] improve message handling from engine
- [x] move engine uci parsing outside of `EngineState`
- [x] implement engine option reading
- [x] implement engine option setting
- [x] implement MultiPV (calculate multiple lines)
- [x] cli: get engine path from cli param or env
- [x] cli: move current logic into `search` subcommand
- [x] cli: new subcommand `list-options` 
- [ ] cli (search): new param `--option/-O` to pass engine options
- [ ] cli (search): new param `--max-depth` to search up to a set depth 
- [ ] more stuff?


## Minor:

- [ ] fix: Handle EngineOption names with spaces
- [ ] fix: Handle OptionType::Combo options with spaces