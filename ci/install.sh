set -ex

main() {
    curl https://sh.rustup.rs -sSf | \
        sh -s -- -y --default-toolchain $TRAVIS_RUST_VERSION

    local host_target=
    if [ $TRAVIS_OS_NAME = linux ]; then
        host_target=x86_64-unknown-linux-gnu
    else
        host_target=x86_64-apple-darwin
    fi

    if [ $host_target != $TARGET ]; then
        ~/.cargo/bin/rustup target add $TARGET
    fi

    ## Not using `cross` for now since it breaks compiling c++.. Being worked on in japaric/cross PR 45
    # At some point you'll probably want to use a newer release of `cross`,
    # simply change the argument to `--tag`.
    # curl -LSfs https://japaric.github.io/trust/install.sh | \
    #      sh -s -- \
    #        --force \
    #        --git japaric/cross \
    #        --tag v0.1.4 \
    #        --target $target

}

main
