# GUI-Experiment
<br>

1. Create a virtual env using venv or uv.
   
   Note: I recommend using uv, its awesome and streamline things in just one.  

* Using venv:
    ```bash
    python -m venv venv
    ```

* Using uv:
    ```bash
    uv venv
    ```
<br>

2. Installing dependencies.
            
    Note: Any uv commands, you don't need to active virtual env, it will automatically finds the venv in current dir and activates it to execute the command in the venv.

    * It allows you to install the dependencies same way you do with pip.
        >   ```bash
        >   uv pip install pydantic maturin websockets orjson isort aiomultiprocess aiofiles
        >   ```

        or <br>
    
    * This command installs dependencies from pyproject.toml file by resolving the version constraints.
        >    ```bash
        >    uv pip install -r pyproject.toml
        >    ```
        
        or <br>

    * This command sync venv with pyproject.toml file, which installs dependencies which are not installed and removes which are not defined in pyproject.toml file.
        >    ```bash
        >    uv sync
        >    ```
<br>

3. Compile rust package.
    * It builds the wheel package including the rust binary and installs it as editable project. 
        
        Note: if pyproject.toml contains maturin as build system and if you run `uv sync` then you don't need do run this command. <br>
        
        >    ```bash
        >    maturin develop
        >    ```
<br>

