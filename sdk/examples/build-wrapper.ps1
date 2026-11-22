# Wraps a build command and shows it as a live activity.
#   .\build-wrapper.ps1 cargo build --release
#   .\build-wrapper.ps1 npm run build
$id = "build-$PID"
$name = $args[0]
notchly live start --id $id --title "Build" --subtitle "$name running" --icon hammer --action "cancel:Cancel" | Out-Null
$sw = [Diagnostics.Stopwatch]::StartNew()
& $args[0] $args[1..($args.Length - 1)]
$ok = $LASTEXITCODE -eq 0
$t = "{0}m {1}s" -f [int]$sw.Elapsed.TotalMinutes, $sw.Elapsed.Seconds
if ($ok) { notchly live end --id $id --final "Build done in $t" | Out-Null }
else { notchly live end --id $id --final "Build failed after $t" --linger 8 | Out-Null }
exit $LASTEXITCODE
