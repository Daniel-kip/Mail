# Mailvora web frontend: server-rendered PHP behind Apache.
# The official image already includes mbstring and openssl; curl and
# opcache are added here. No Composer packages are required at runtime.
FROM php:8.5-apache

RUN apt-get update \
 && apt-get install -y --no-install-recommends libcurl4-openssl-dev \
 && docker-php-ext-install curl opcache \
 && a2enmod rewrite headers remoteip \
 && rm -rf /var/lib/apt/lists/*

COPY docker/apache-vhost.conf /etc/apache2/sites-available/000-default.conf
COPY php.ini /usr/local/etc/php/conf.d/mailvora.ini
WORKDIR /var/www/mailvora
COPY . /var/www/mailvora
RUN rm -f .env \
 && mkdir -p storage/sessions storage/logs \
 && chown -R root:root /var/www/mailvora \
 && chown -R www-data:www-data storage \
 && chmod -R 0750 storage

ENV APP_ENV=production \
    SESSION_SECURE=true \
    TRUST_PROXY=true

HEALTHCHECK --interval=30s --timeout=5s --retries=3 CMD php -r "exit(@file_get_contents('http://127.0.0.1/healthz') === false ? 1 : 0);"
EXPOSE 80
