package mescript;

import clojure.lang.RT;
import clojure.lang.Var;
import java.awt.GraphicsEnvironment;
import java.io.File;
import java.net.URISyntaxException;

public final class MescriptWorkstation {
    private MescriptWorkstation() {
    }

    public static void main(String[] args) throws Exception {
        if (args.length > 0 && ("--help".equals(args[0]) || "-h".equals(args[0]))) {
            usage();
            return;
        }
        if (GraphicsEnvironment.isHeadless()) {
            System.err.println("error: no graphical display is available for the Swing workstation");
            System.err.println("run ./glitchlisp-native edit [file.gl] for the terminal editor");
            System.exit(1);
        }
        System.setProperty("mescript.app.dir", appDir());
        Var.pushThreadBindings(
                RT.map(RT.var("clojure.core", "*command-line-args*"), RT.seq(args)));
        try {
            try {
                RT.loadResourceScript("main.clj");
            } catch (Throwable error) {
                if (isDisplayError(error)) {
                    System.err.println("error: unable to open the Swing display");
                    System.err.println(rootMessage(error));
                    System.err.println("run ./glitchlisp-native edit [file.gl] for the terminal editor");
                    System.exit(1);
                }
                if (error instanceof Exception) {
                    throw (Exception) error;
                }
                throw (Error) error;
            }
        } finally {
            Var.popThreadBindings();
        }
    }

    private static void usage() {
        System.out.println("usage:");
        System.out.println("  java -jar mescript.jar [file.gl]");
        System.out.println();
        System.out.println("Opens the Swing workstation.");
        System.out.println("For terminal tools, use ./glitchlisp-native.");
    }

    private static boolean isDisplayError(Throwable error) {
        for (Throwable current = error; current != null; current = current.getCause()) {
            String message = current.getMessage();
            if (current instanceof java.awt.AWTError) {
                return true;
            }
            if (message != null
                    && (message.contains("Can't connect to X11")
                    || message.contains("DISPLAY")
                    || message.contains("no X11 DISPLAY"))) {
                return true;
            }
        }
        return false;
    }

    private static String rootMessage(Throwable error) {
        Throwable current = error;
        while (current.getCause() != null) {
            current = current.getCause();
        }
        String message = current.getMessage();
        return message == null ? current.toString() : message;
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
