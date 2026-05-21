# loc

counts lines of code in a git repo or workspace.

## install

```bash
make install
```

this builds the release binary and copies it to `/usr/local/bin/loc`.

## usage

```bash
# count LOC in the current directory
loc

# count LOC in a specific path
loc ~/dev/myproject
```

if the target is a git repo, it counts that single repo.
if it contains multiple git repos, it lists each one separately.

ignores `target/`, `node_modules/`, and anything in `.gitignore`.

## uninstall

```bash
make uninstall
```
