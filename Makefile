.PHONY: build test deploy-testnet deploy-mainnet

build:
	@bash scripts/build.sh

test:
	@bash scripts/test.sh

deploy-testnet:
	@bash scripts/deploy.sh --network testnet

deploy-mainnet:
	@echo "WARNING: You are about to deploy to MAINNET."
	@echo "This action is irreversible. Type 'yes' to continue:"
	@read confirm && [ "$$confirm" = "yes" ] || (echo "Aborted." && exit 1)
	@bash scripts/deploy.sh --network mainnet
