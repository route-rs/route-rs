echo "Symlinking .githooks directory..."
git config core.hooksPath .githooks

echo "Installing Rust style tools..."
rustup update
rustup component add rustfmt
rustup component add clippy