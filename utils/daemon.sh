#! /bin/bash

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

while true
do
	cd "$script_dir"
	
	cd ..
	/usr/bin/git fetch --all
	/usr/bin/git reset --hard origin/main

	$HOME/.cargo/bin/cargo run
done
