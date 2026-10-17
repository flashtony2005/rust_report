.PHONY: install-local install-local-legacy check
install-local:
	cargo install --path crates/rpx-cli --locked --root "$(HOME)/.local"

install-local-legacy:
	cargo install --path crates/rpx-cli --locked --features legacy-rpx --root "$(HOME)/.local"
	@printf '\nInstalled: %s/.local/bin/rpx\nAdd $$HOME/.local/bin to PATH when needed.\n' "$(HOME)"
check:
	bash scripts/gate.sh
