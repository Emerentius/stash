# stash

Store any command's output by piping it into `stash` and retrieve it later. It's like `some_command > myfile` but without most of the hassle of managing `myfile`.

# Usage

```bash
# Store stdout of some command
$ echo First | stash
$ echo Second | stash push  # equivalent to above

# Retrieve the outputs again
$ stash show # or stash show 0
Second

$ stash show 1
First

# Show all stashed outputs.
$ stash list
1: 2022-06-18T00:01:29.555478344Z
0: 2022-06-18T00:01:25.667445118Z

# Show latest stash and delete it afterwards
$ stash pop
Second

# Delete all stashed outputs
$ stash clear
```
