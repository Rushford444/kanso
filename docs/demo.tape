Output docs/demo.gif
Set Width 1100
Set Height 600
Type "demo=$(mktemp -d); cd \"$demo\""
Enter
Type "printf '# Project instructions\\n\\n## Build\\n\\nSee [build guide](missing.md).\\n' > AGENTS.md"
Enter
Type "kanso check ."
Enter
Sleep 2s
Type "kanso score ."
Enter
Sleep 2s
Type "kanso explain KS004"
Enter
Sleep 2s
Type "cd /; rm -rf \"$demo\""
Enter
