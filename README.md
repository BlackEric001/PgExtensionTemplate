# PgExtensionTemplate
Шаблон расширения для PostgreSQL на языке Rust для валидации SQL-запросов. 

Для работы нужен PostgreSQL 18.x

Для сборки под Windows 10 запустить build_pg_query_validator.bat. Скрипт проверяет установленное окружение и собирает расширение.

После успешной работы  build_pg_query_validator.bat для установки расширения нужно запустить с правами администратора install_pg_query_validator.bat. Он произведет установку расширения.

После установки выполнить CREATE EXTENSION IF NOT EXISTS pg_query_validator;

Затем проверить, что PostgreSQL видит расширение:
SELECT name, default_version, installed_version
FROM pg_available_extensions
WHERE name = 'pg_query_validator';

Запрос SELECT get_config(); должен вернуть json с текущими настройками валидатора

Для ручной проверки запроса выполнить SELECT validate_query('SELECT 1');

При повторной установке расширения необходимо остановить службу PostgreSQL. В противном случае Windows не даст заменить файлы.
