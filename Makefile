.PHONY: all build build-dev push pull run clean

# Параметры
IMAGE_NAME = wepal/rust-ssl-checker
DOCKERFILE = Dockerfile
CONTEXT = .

# Получаем версию из Cargo.toml
CARGO_VERSION := $(shell grep '^version' checker/Cargo.toml | cut -d '"' -f 2)
ifeq ($(CARGO_VERSION),)
$(error Не удалось получить версию из Cargo.toml. Убедитесь, что `jq` \
установлен, путь к Cargo.toml (checker/Cargo.toml) и имя пакета в Cargo.toml ("rust-app") указаны верно.)
endif

IMAGE = $(IMAGE_NAME):$(CARGO_VERSION)

all: show_version build

show_version:
	@echo "Версия из Cargo.toml: $(CARGO_VERSION)"

build:
	@docker build -f $(DOCKERFILE) -t $(IMAGE) $(CONTEXT)

build-dev:
	@docker build -f $(DOCKERFILE).dev -t $(IMAGE) $(CONTEXT)

run:
	@docker run --rm $(IMAGE)

push:
	@docker push $(IMAGE)

pull:
	@docker pull $(IMAGE)

clean:
	@docker rmi $(IMAGE) || true