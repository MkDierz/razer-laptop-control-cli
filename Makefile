NAME    := razer-cli
VERSION := 0.3.0

PREFIX      ?= /usr/local
BINDIR      ?= $(PREFIX)/bin
MANDIR      ?= $(PREFIX)/share/man/man1
UDEVDIR     ?= $(PREFIX)/lib/udev/rules.d
SYSTEMD_DIR ?= $(PREFIX)/lib/systemd/user
DESTDIR     ?=

CARGO    := cargo
INSTALL  := install
FPM      := fpm
ARCH     := $(shell uname -m)

ifeq ($(ARCH),x86_64)
  DEB_ARCH := amd64
else ifeq ($(ARCH),aarch64)
  DEB_ARCH := arm64
else
  DEB_ARCH := $(ARCH)
endif

PKGDIR := $(CURDIR)/target/pkg
PKG_PREFIX := /usr

.PHONY: all build man install uninstall clean help \
        deb rpm pacman pkg-clean

all: build

build:
	$(CARGO) build --release

man: docs/razer-cli.1

install: build man
	$(INSTALL) -Dm 0755 target/release/$(NAME)  $(DESTDIR)$(BINDIR)/$(NAME)
	$(INSTALL) -Dm 0644 docs/razer-cli.1        $(DESTDIR)$(MANDIR)/$(NAME).1
	$(INSTALL) -Dm 0644 data/udev/99-razer.rules $(DESTDIR)$(UDEVDIR)/99-razer.rules
	$(INSTALL) -Dm 0644 packaging/razer-restore.service $(DESTDIR)$(SYSTEMD_DIR)/razer-restore.service

uninstall:
	rm -f $(DESTDIR)$(BINDIR)/$(NAME)
	rm -f $(DESTDIR)$(MANDIR)/$(NAME).1
	rm -f $(DESTDIR)$(UDEVDIR)/99-razer.rules
	rm -f $(DESTDIR)$(SYSTEMD_DIR)/razer-restore.service

clean:
	$(CARGO) clean
	rm -rf $(PKGDIR)

# --- FPM packaging -----------------------------------------------------------

pkg-clean:
	rm -rf $(PKGDIR)

$(PKGDIR)/.stamp:
	rm -rf $(PKGDIR)
	$(MAKE) install DESTDIR=$(PKGDIR) PREFIX=$(PKG_PREFIX)
	@touch $@

deb: $(PKGDIR)/.stamp
	$(FPM) -t deb \
		-p $(PKGDIR)/$(NAME)_$(VERSION)-1_$(DEB_ARCH).deb \
		--architecture $(DEB_ARCH) \
		--depends libudev0 \
		--depends libusb-1.0-0 \
		-C $(PKGDIR) \
		.

rpm: $(PKGDIR)/.stamp
	$(FPM) -t rpm \
		-p $(PKGDIR)/$(NAME)-$(VERSION)-1.$(ARCH).rpm \
		--architecture $(ARCH) \
		--depends systemd \
		--depends libusb1 \
		-C $(PKGDIR) \
		.

pacman: $(PKGDIR)/.stamp
	$(FPM) -t pacman \
		-p $(PKGDIR)/$(NAME)-$(VERSION)-1-$(ARCH).pkg.tar.zst \
		--architecture $(ARCH) \
		--depends systemd \
		--depends libusb \
		-C $(PKGDIR) \
		.

# --- End FPM packaging -------------------------------------------------------

help:
	@echo "Targets:"
	@echo "  build    - Build release binary (default)"
	@echo "  man      - Man page (static, always up to date)"
	@echo "  install  - Install binary, man page, udev rule, systemd unit"
	@echo "  uninstall- Remove installed files"
	@echo "  deb      - Build .deb package (FPM)"
	@echo "  rpm      - Build .rpm package (FPM)"
	@echo "  pacman   - Build .pkg.tar.zst package (FPM)"
	@echo "  pkg-clean- Remove package staging directory"
	@echo "  clean    - Remove build and package artifacts"
	@echo ""
	@echo "Variables:"
	@echo "  PREFIX=$(PREFIX)  BINDIR=$(BINDIR)"
	@echo "  DESTDIR=$(DESTDIR)  (for staging installs)"
	@echo "  ARCH=$(ARCH)  DEB_ARCH=$(DEB_ARCH)"
