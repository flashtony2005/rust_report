param([string]$Mode = "gen")
# Fixture generator / verifier driver (ASCII only -- PowerShell 5.1 parses
# BOM-less .ps1 as ANSI/GBK, so non-ASCII literals would break it).
#   gen    (default): (re)write .rpx + .expected.json from the Java models
#   verify          : round-trip -- re-render each .rpx and compare with the
#                     committed .expected.json (20 fixtures)
# Usage:  powershell -ExecutionPolicy Bypass -File gen.ps1
#         powershell -ExecutionPolicy Bypass -File gen.ps1 -Mode verify
$JAVAC = "C:/Users/Administrator/.workbuddy/binaries/java/jdk17/jdk-17.0.20.1+1/bin/javac.exe"
$JRE = "F:/project/report/common/jre/bin/java.exe"
$LIB = "F:/project/report/reportlite/lib"
$FIXLIB = "F:/project/report/rpx/tests/fixtures/lib"
$OUT = "F:/project/report/rpx/tests/fixtures/out"
$SRC = "F:/project/report/rpx/tests/fixtures"

$libjars = (Get-ChildItem "$LIB\*.jar").FullName -join ';'
$fixjars = (Get-ChildItem "$FIXLIB\*.jar").FullName -join ';'
$CP = "$OUT;$libjars;$fixjars"

Write-Host "CP=$CP"
Write-Host "MODE=$Mode"

& $JAVAC -encoding UTF-8 --release 8 -cp $CP -d $OUT "$SRC\*.java" 2>&1 | Out-File -Encoding UTF8 "$SRC\out\_compile.log"
if ($LASTEXITCODE -ne 0) {
    Write-Host "COMPILE FAILED exit=$LASTEXITCODE"
    exit $LASTEXITCODE
}
Write-Host "COMPILE OK"

Set-Location $SRC
if ($Mode -eq "verify") {
    $runCmd = "$JRE -Dfile.encoding=UTF-8 -cp `"$CP`" rpx.fixtures.GenEngineFixtures --verify ."
    cmd /c $runCmd 2>&1 | Out-File -Encoding UTF8 "$SRC\out\_verify.log"
    Write-Host "VERIFY exit=$LASTEXITCODE"
    exit $LASTEXITCODE
} else {
    $runCmd = "$JRE -Dfile.encoding=UTF-8 -cp `"$CP`" rpx.fixtures.GenEngineFixtures ."
    cmd /c $runCmd 2>&1 | Out-File -Encoding UTF8 "$SRC\out\_gen.log"
    Write-Host "GEN exit=$LASTEXITCODE"
    exit $LASTEXITCODE
}
