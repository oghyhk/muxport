param()

$ErrorActionPreference = 'Stop'

$protocVersion = '34.2'
$dartPluginVersion = '25.0.0'
$repositoryRoot = Split-Path -Parent $PSScriptRoot
$schemaDirectory = Join-Path (Join-Path $repositoryRoot 'protocol') 'schema'
$outputDirectory = Join-Path (Join-Path (Join-Path $repositoryRoot 'packages') 'flutter_protocol') 'lib'
$outputDirectory = Join-Path (Join-Path $outputDirectory 'src') 'generated'
$schemaFile = Join-Path $schemaDirectory 'muxport.proto'
$toolRoot = Join-Path (Join-Path $repositoryRoot '.tools') "protoc-$protocVersion"

$runningOnWindows = [System.Runtime.InteropServices.RuntimeInformation]::IsOSPlatform(
  [System.Runtime.InteropServices.OSPlatform]::Windows
)
$runningOnLinux = [System.Runtime.InteropServices.RuntimeInformation]::IsOSPlatform(
  [System.Runtime.InteropServices.OSPlatform]::Linux
)
$runningOnMacOS = [System.Runtime.InteropServices.RuntimeInformation]::IsOSPlatform(
  [System.Runtime.InteropServices.OSPlatform]::OSX
)
$architecture = [System.Runtime.InteropServices.RuntimeInformation]::ProcessArchitecture.ToString()

if ($runningOnWindows -and $architecture -eq 'X64') {
  $assetName = "protoc-$protocVersion-win64.zip"
  $expectedDigest = 'b6dbc741897760694630ca9ee6a22fda589647b573fd6843d6943b72ceb05c15'
} elseif ($runningOnLinux -and $architecture -eq 'X64') {
  $assetName = "protoc-$protocVersion-linux-x86_64.zip"
  $expectedDigest = 'b52e803fad2f63232f75351c0ff735e891f40262de791bade78a3636831a522a'
} elseif ($runningOnMacOS -and $architecture -eq 'Arm64') {
  $assetName = "protoc-$protocVersion-osx-aarch_64.zip"
  $expectedDigest = '90ced886e57a96a1ec98bf1f23bef2532bb1636ca8b27bb5eb34fea89cd6ef8b'
} elseif ($runningOnMacOS -and $architecture -eq 'X64') {
  $assetName = "protoc-$protocVersion-osx-x86_64.zip"
  $expectedDigest = '8538ec43139ce2759ffd0840954cc0415796934b20c2a86fe493c9af2277cbea'
} else {
  throw "Unsupported protocol generation platform/architecture: $architecture"
}

$archive = Join-Path $toolRoot $assetName
$protocName = if ($runningOnWindows) { 'protoc.exe' } else { 'protoc' }
$protocPath = Join-Path (Join-Path $toolRoot 'bin') $protocName
if (-not (Test-Path -LiteralPath $archive)) {
  New-Item -ItemType Directory -Path $toolRoot -Force | Out-Null
  $downloadUrl = "https://github.com/protocolbuffers/protobuf/releases/download/v$protocVersion/$assetName"
  Invoke-WebRequest -Uri $downloadUrl -OutFile $archive
}
$actualDigest = (Get-FileHash -LiteralPath $archive -Algorithm SHA256).Hash.ToLowerInvariant()
if ($actualDigest -ne $expectedDigest) {
  throw "protoc archive digest mismatch for $assetName"
}
if (-not (Test-Path -LiteralPath $protocPath)) {
  Expand-Archive -LiteralPath $archive -DestinationPath $toolRoot -Force
}
if (-not $runningOnWindows) {
  & chmod +x $protocPath
  if ($LASTEXITCODE -ne 0) {
    throw 'could not mark protoc executable'
  }
}

$activePackages = (& dart pub global list) -join "`n"
if ($activePackages -notmatch "(?m)^protoc_plugin\s+$([regex]::Escape($dartPluginVersion))$") {
  & dart pub global activate protoc_plugin $dartPluginVersion
  if ($LASTEXITCODE -ne 0) {
    throw 'could not activate the pinned Dart protoc plugin'
  }
}
$userProfile = [Environment]::GetFolderPath([Environment+SpecialFolder]::UserProfile)
$pubCache = if ($env:PUB_CACHE) {
  $env:PUB_CACHE
} elseif ($runningOnWindows) {
  Join-Path (Join-Path $env:LOCALAPPDATA 'Pub') 'Cache'
} else {
  Join-Path $userProfile '.pub-cache'
}
$env:PATH = (Join-Path $pubCache 'bin') + [IO.Path]::PathSeparator + $env:PATH
$dartPlugin = Get-Command protoc-gen-dart -ErrorAction Stop

New-Item -ItemType Directory -Path $outputDirectory -Force | Out-Null
& $protocPath `
  "--plugin=protoc-gen-dart=$($dartPlugin.Source)" `
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
