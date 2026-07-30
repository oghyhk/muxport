param()

$ErrorActionPreference = 'Stop'

$repositoryRoot = Split-Path -Parent $PSScriptRoot
$schemaDirectory = Join-Path $repositoryRoot 'protocol\schema'
$outputDirectory = Join-Path $repositoryRoot 'packages\flutter_protocol\lib\src\generated'
$schemaFile = Join-Path $schemaDirectory 'muxport.proto'

$protoc = Get-Command protoc -ErrorAction Stop
$null = Get-Command protoc-gen-dart -ErrorAction Stop
New-Item -ItemType Directory -Path $outputDirectory -Force | Out-Null

& $protoc.Source `
  "--proto_path=$schemaDirectory" `
  "--dart_out=$outputDirectory" `
  $schemaFile
if ($LASTEXITCODE -ne 0) {
  throw "protoc failed with exit code $LASTEXITCODE"
}

& dart format $outputDirectory
if ($LASTEXITCODE -ne 0) {
  throw "dart format failed with exit code $LASTEXITCODE"
}
