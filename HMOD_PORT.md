# hmod port: noteblock
Round 1
- Fixed: src/lib.rs: the command check used `permission_value` in the engine's closure (1 error). Now `commands::needs(&line)` names the matched node, `permission_bool` asks it (before the engine lock; the engine closure is `Fn`, so the query cannot run inside it), and any error is a denial. The `op_level` lookup is gone (the node default replaces it).
- Fixed: src/lib.rs: added `permission_gather_nodes` with `permission_nodes()`, which declares every node named by `commands::usages()` as a Boolean node.
- Permission nodes: all nodes from `commands::usages()` (`radio.toggle`, `radio.volume`, `{worldradio,serverradio,positionradio}.{admin,enable,skip}`, `nbmp.admin`, ...) -> default = the usage's operator level (0 or 1). Declared in `permission_nodes()` / `permission_gather_nodes` in src/lib.rs.
- Gaps: none
- Unsure: (1) when several usages name one node with different levels, the first usage's level is the default (the usages agree today); (2) the `needs` lookup runs once more than the engine's own check, same result.
