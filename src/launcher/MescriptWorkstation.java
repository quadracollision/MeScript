package mescript;

import clojure.lang.RT;
import clojure.lang.Var;
import java.io.File;
import java.net.URISyntaxException;

public final class MescriptWorkstation {
    private MescriptWorkstation() {
    }

    public static void main(String[] args) throws Exception {
        System.setProperty("mescript.app.dir", appDir());
        Var.pushThreadBindings(
                RT.map(RT.var("clojure.core", "*command-line-args*"), RT.seq(args)));
        try {
            RT.loadResourceScript("main.clj");
        } finally {
            Var.popThreadBindings();
        }
    }

    private static String appDir() throws URISyntaxException {
        File location = new File(MescriptWorkstation.class
                .getProtectionDomain()
                .getCodeSource()
                .getLocation()
                .toURI());
        File dir = location.isFile() ? location.getParentFile() : location;
        return dir.getAbsolutePath();
    }
}
