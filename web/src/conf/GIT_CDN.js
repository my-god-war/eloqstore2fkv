import GIT_REV from "./GIT_REV.js";
import { GIT_RAW } from "./URL.js";

export const GIT_CDN = GIT_RAW + GIT_REV + "/",
  GIT_DOC = GIT_CDN + "web/doc/",
  GIT_WEBC_README_PREFIX = GIT_CDN + "web/src/webc/",
  GIT_WEBC_README_I18N_DIR = "/i18n/",
  GIT_WEBC_README_MD = "README.md",
  GIT_JS_README_I18N_DIR = "/js/",
  GIT_JS_README_MD = ".md",
  GIT_JS_SRC_PREFIX = GIT_CDN + "web/src/js/",
  GIT_JS_SRC_SUFFIX = ".js";
