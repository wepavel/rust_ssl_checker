# Rust SSL Checker
Сервис предназначен для проверки срока действия ssl-сертификатов и информирования об их устаревании.

## Конфигурация
Шаблон файла конфигурации лежит в `config.template.yml`.

Ключи верхнего уровня:
* `alarm_days` - число дней до срока истечения домена, начиная с которого отправляются уведомления (по умолчанию `7`)
* `ssl_alarm_days` - число дней до срока истечения сертификата, начиная с которого отправляются уведомления (по умолчанию `7`)
* `check_interval_hours` - число часов между проверками (по умолчанию `7`)
* `sources` - источники доменов для проверки
* `notifiers` - модули отправки уведомлений

## Конфигурация логгирования
```yaml
log_config:
  # Уровень логгирования
  # trace, debug, info, warn, error
  log_level: info
  use_color: true
```

## Источники доменов

### Текстовый файл
```yaml
sources:
  filename: "hostnames.txt"
```
### Selectel
```yaml
sources:
  selectel:
    account_id: "12345"
    password: "password"
    project_name: "Project Name"
    user: "user"
```

## Модули уведомлений

### Вывод в консоль
```yaml
notifiers:
  console: ~
```

### Telegram
Параметр `retries` является опциональным
```yaml
notifiers:
  telegram:
    bot_token: "1231231231:WASDwasd..."
    chat_id: "-1231231231"
    retries: 5
```

## Запуск
### Терминал
```bash
./checker
```
### Из запущенного Docker-контейнера
```bash
docker exec <container_name> /app/checker single_shot
```
### По расписанию
```yaml
name: ssl-checker

services:
  ssl-checker:
    image: "wepal/rust-ssl-checker:latest"
    restart: "always"
    volumes:
      - /etc/localtime:/etc/localtime:ro
      - ./config.yaml:/app/config.yml:ro
      - ./hostnames.txt:/app/hostnames.txt:ro
```

## Сборка образа
```bash
make
```