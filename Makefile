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
IMAGE_LATEST = $(IMAGE_NAME):latest

all: show_version build

show_version:
	@echo "Версия из Cargo.toml: $(CARGO_VERSION)"

build:
	@docker build -f $(DOCKERFILE) -t $(IMAGE) $(CONTEXT)
	@docker build -f $(DOCKERFILE) -t $(IMAGE_LATEST) $(CONTEXT)

run:
	@docker run --rm $(IMAGE)

docker-login:
	@echo "Logging in to Docker registry..."
	@docker login -u $(DOCKER_USER) -p $(DOCKER_PASS)

push: docker-login
	@docker push $(IMAGE)

push-latest: docker-login
	@echo "Pushing latest image: $(IMAGE_LATEST)"
	@docker push $(IMAGE_LATEST)

pull:
	@docker pull $(IMAGE)

clean:
	@docker rmi $(IMAGE) || true