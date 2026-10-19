@echo off
rem Starts the whole local stack (backend dependencies, Rust API and this PHP
rem frontend) through the root launcher, which also loads the PHP extensions.
call "%~dp0..\start-local.cmd" %*
