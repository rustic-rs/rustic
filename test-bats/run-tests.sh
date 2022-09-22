#!/bin/bash
dir="$( cd -- "$( dirname -- "${BASH_SOURCE[0]}"  )" &> /dev/null && pwd )"
# install bats
git clone https://github.com/bats-core/bats-core.git $dir/bats
git clone https://github.com/bats-core/bats-support.git $dir/bats-support
git clone https://github.com/bats-core/bats-assert.git $dir/bats-assert
PATH=$PATH:$dir/bats/bin
# run tests
bats $dir
# clean up
rm -rf $dir/bats
rm -rf $dir/bats-support
rm -rf $dir/bats-assert

