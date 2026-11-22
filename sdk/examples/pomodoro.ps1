# A terminal Pomodoro that lives in the notch.  .\pomodoro.ps1 25
param([int]$Minutes = 25)
$id = "pomodoro-term"
$total = $Minutes * 60
notchly live start --id $id --title "Focus" --icon timer --progress 0 --priority 70 | Out-Null
for ($s = 0; $s -le $total; $s += 5) {
  $left = $total - $s
  notchly live update --id $id --subtitle ("{0}:{1:D2} left" -f [int][math]::Floor($left / 60), ($left % 60)) --progress ($s / $total) | Out-Null
  Start-Sleep -Seconds 5
}
notchly live end --id $id --final "Focus done — take a break" --linger 10 | Out-Null
