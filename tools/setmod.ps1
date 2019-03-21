# This is a helper script to run setmod in a loop, restarting if it shuts down.
# It can be evoked from anywhere on the filesystem.

. "$PSScriptRoot\env.ps1"

while($true) {
    cargo run --manifest-path="$PSScriptRoot\..\bot\Cargo.toml"
    Write-Host "Bot shut down, restarting in 5s..."
    Start-Sleep -s 5
}