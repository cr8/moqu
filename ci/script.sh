# This script takes care of testing your crate

set -ex

CARGO=~/.cargo/bin/cargo

main() {
    test -f Cargo.lock || cargo generate-lockfile

    $CC_X --version
    $CXX_X --version
    export CC=${CC_X}
    export CXX=${CXX_X}

    mkdir .cargo
    echo "[target.$TARGET]" > .cargo/config
    echo "linker=\"$CC\"" >> .cargo/config
    cat .cargo/config

    $CARGO build --target $TARGET
    $CARGO build --target $TARGET --release

    if [ -z $DISABLE_TESTS ]; then
        $CARGO test --target $TARGET
        $CARGO test --target $TARGET --release

        # cargo run --target $TARGET
        # cargo run --target $TARGET --release
    fi
}

# we don't run the "test phase" when doing deploys
if [ -z $TRAVIS_TAG ]; then
    main
fi
