# creativecraft

The applications that make up CreativeCraft.

| Folder | What it is |
|---|---|
| `studio-shell/` | The window you launch: bottom tab bar, workspace hosting, menu bar, OpenFX helper process |
| `effectcraft/` | Effects app: node graph, keyframes, OpenFX host |
| `filmcraft/` | Edit, Audio and Render |
| `photocraft/` | Photo editor |
| `figcraft/` | Design tool, shown from Photo's link panel |
| `vectorcraft/` | Vector editor |
| `soundcraft/` | Sound tools |
| `solidcraft/` | Not hosted — it uses a different egui version |
| `libs/` | Shared crates used by the apps |

Start with `studio-shell/`. To run the whole thing:

```powershell
cd studio-shell
cargo run
```

Do not edit the other folders to add CreativeCraft features; the shell hosts
them and draws its own chrome around them. See the top-level README for the
full description, limits and licences.
