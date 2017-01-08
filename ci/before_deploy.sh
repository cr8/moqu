# This script takes care of building your crate and packaging it for release

set -ex

main() {
    local src=$(pwd) \
          stage=

    case $TRAVIS_OS_NAME in
        linux)
            stage=$(mktemp -d)
            ;;
        osx)
            stage=$(mktemp -d -t tmp)
            ;;
    esac

    test -f Cargo.lock || cargo generate-lockfile

    $CC_X --version
    $CXX_X --version
    export CC=${CC_X}
    export CXX=${CXX_X}

    mkdir .cargo
    echo "[target.$TARGET]" > .cargo/config
    echo "linker=\"$CC\"" >> .cargo/config
    cat .cargo/config
    
    # Update this to build the artifacts that matter to you
    cargo rustc --bin moqu --target $TARGET --release -- -C lto

    # Update this to package the right artifacts
    cp target/$TARGET/release/moqu $stage/

    cd $stage
    tar czf $src/$CRATE_NAME-$TRAVIS_TAG-$TARGET.tar.gz *
    cd $src

    rm -rf $stage
}

main
