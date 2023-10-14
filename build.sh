#!/bin/bash
echo "Start building"


WIN="x86_64-pc-windows-gnu"
MAC_SILICON="aarch64-apple-darwin"
MAC_INTEL="x86_64-apple-darwin"
BIN_NAME="league_of_legend_pick_helper"

array=($WIN $MAC_SILICON $MAC_INTEL)

for item in ${array[@]};
do
    printf "   %s\n" $item
    echo "Starting build for $item"
    cargo build --profile release --target=$item
    echo "Build finish for $item"

done

cp "./target/$WIN/release/$BIN_NAME.exe" "./dist/${BIN_NAME}_windows.exe"
cp "./target/$MAC_SILICON/release/$BIN_NAME" "./dist/${BIN_NAME}_mac_silicon"
cp "./target/$MAC_INTEL/release/$BIN_NAME" "./dist/${BIN_NAME}_mac_intel"

echo "build finish"